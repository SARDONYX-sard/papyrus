// SPDX-FileCopyrightText: (C) 2025 russo-2025
// SPDX-License-Identifier: MIT
//! Reading `.pex` files.

use std::borrow::Cow;

use crate::{Error, pex::*};

/// A cursor over the bytes of a file, big-endian.
struct Reader<'a> {
    original: &'a [u8],
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn offset(&self) -> usize {
        self.original.len() - self.remaining.len()
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let Some((head, tail)) = self.remaining.split_at_checked(len) else {
            return Err(Error::UnexpectedEof {
                offset: self.offset(),
                expected: len,
                remaining: self.remaining.len(),
            });
        };
        self.remaining = tail;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        Ok(self.take(N)?.try_into().expect("N bytes"))
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_be_bytes(self.array()?))
    }

    fn i64(&mut self) -> Result<i64, Error> {
        Ok(i64::from_be_bytes(self.array()?))
    }

    fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn string(&mut self) -> Result<PexStr<'a>, Error> {
        let len = self.u16()? as usize;
        Ok(PexStr(Cow::Borrowed(self.take(len)?)))
    }

    fn string_id(&mut self) -> Result<StringId, Error> {
        Ok(StringId(self.u16()?))
    }

    fn vec<T>(
        &mut self,
        mut f: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<Vec<T>, Error> {
        let len = self.u16()? as usize;
        (0..len).map(|_| f(self)).collect()
    }

    fn value(&mut self) -> Result<VariableValue, Error> {
        let offset = self.offset();
        Ok(match self.u8()? {
            0 => VariableValue::Null,
            1 => VariableValue::Identifier(self.string_id()?),
            2 => VariableValue::String(self.string_id()?),
            3 => VariableValue::Integer(self.i32()?),
            4 => VariableValue::Float(self.f32()?),
            5 => VariableValue::Boolean(self.u8()? != 0),
            value => return Err(Error::InvalidVariableValue { offset, value }),
        })
    }

    fn variable_type(&mut self) -> Result<VariableType, Error> {
        Ok(VariableType { name: self.string_id()?, ty: self.string_id()? })
    }

    fn instruction(&mut self) -> Result<Instruction, Error> {
        let offset = self.offset();
        let opcode = OpCode::try_from(self.u8()?)
            .map_err(|source| Error::InvalidOpCode { offset, source })?;
        let mut args = Vec::with_capacity(opcode.argument_count() + 1);
        for _ in 0..opcode.argument_count() {
            args.push(self.value()?);
        }
        if opcode.is_variadic() {
            let count = self.value()?;
            args.push(count);
            if let VariableValue::Integer(n) = count {
                for _ in 0..n {
                    args.push(self.value()?);
                }
            }
        }
        Ok(Instruction { opcode, args })
    }

    fn function_info(&mut self) -> Result<FunctionInfo, Error> {
        Ok(FunctionInfo {
            return_type: self.string_id()?,
            docstring: self.string_id()?,
            user_flags: self.u32()?,
            flags: FunctionFlags::from_bits_retain(self.u8()?),
            params: self.vec(Self::variable_type)?,
            locals: self.vec(Self::variable_type)?,
            instructions: self.vec(Self::instruction)?,
        })
    }

    fn property(&mut self) -> Result<Property, Error> {
        let name = self.string_id()?;
        let ty = self.string_id()?;
        let docstring = self.string_id()?;
        let user_flags = self.u32()?;
        let flags = PropertyFlags::from_bits_retain(self.u8()?);
        let (auto_var_name, read_handler, write_handler) =
            if flags.contains(PropertyFlags::AUTO_VAR) {
                (Some(self.string_id()?), None, None)
            } else {
                let read = if flags.contains(PropertyFlags::READ) {
                    Some(self.function_info()?)
                } else {
                    None
                };
                let write = if flags.contains(PropertyFlags::WRITE) {
                    Some(self.function_info()?)
                } else {
                    None
                };
                (None, read, write)
            };
        Ok(Property {
            name,
            ty,
            docstring,
            user_flags,
            flags,
            auto_var_name,
            read_handler,
            write_handler,
        })
    }

    fn object(&mut self) -> Result<Object, Error> {
        Ok(Object {
            name: self.string_id()?,
            size: self.u32()?,
            parent_class_name: self.string_id()?,
            docstring: self.string_id()?,
            user_flags: ObjectFlags::from_bits_retain(self.u32()?),
            auto_state_name: self.string_id()?,
            variables: self.vec(|r| {
                Ok(Variable {
                    name: r.string_id()?,
                    type_name: r.string_id()?,
                    user_flags: r.u32()?,
                    value: r.value()?,
                })
            })?,
            properties: self.vec(Self::property)?,
            states: self.vec(|r| {
                Ok(State {
                    name: r.string_id()?,
                    functions: r
                        .vec(|r| Ok(Function { name: r.string_id()?, info: r.function_info()? }))?,
                })
            })?,
        })
    }

    fn debug_info(&mut self) -> Result<DebugInfo, Error> {
        Ok(DebugInfo {
            modification_time: self.i64()?,
            functions: self.vec(|r| {
                let object_name = r.string_id()?;
                let state_name = r.string_id()?;
                let function_name = r.string_id()?;
                let offset = r.offset();
                let function_type = match r.u8()? {
                    0 => DebugFunctionType::Method,
                    1 => DebugFunctionType::Getter,
                    2 => DebugFunctionType::Setter,
                    value => return Err(Error::InvalidDebugFunctionType { offset, value }),
                };
                Ok(DebugFunction {
                    object_name,
                    state_name,
                    function_name,
                    function_type,
                    instruction_line_numbers: r.vec(Self::u16)?,
                })
            })?,
        })
    }
}

impl<'a> PexFile<'a> {
    /// Reads a file. Strings borrow `bytes`; see [`PexFile::into_owned`].
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Error> {
        let mut r = Reader { original: bytes, remaining: bytes };
        let magic_number = r.u32()?;
        if magic_number != LE_MAGIC_NUMBER {
            return Err(Error::InvalidMagicNumber { offset: 0, value: magic_number });
        }
        let major_version = r.u8()?;
        let minor_version = r.u8()?;
        let offset = r.offset();
        let game_id = match r.u16()? {
            1 => GameType::Skyrim,
            value => return Err(Error::InvalidGameType { offset, value }),
        };
        let compilation_time = r.i64()?;
        let src_file_name = r.string()?;
        let user_name = r.string()?;
        let machine_name = r.string()?;
        let string_table = r.vec(Reader::string)?;
        let debug_info = if r.u8()? != 0 { Some(r.debug_info()?) } else { None };
        let user_flags = r.vec(|r| Ok(UserFlag { name: r.string_id()?, flag_index: r.u8()? }))?;
        let objects = r.vec(Reader::object)?;
        if !r.remaining.is_empty() {
            return Err(Error::TrailingBytes { offset: r.offset(), len: r.remaining.len() });
        }
        Ok(PexFile {
            magic_number,
            major_version,
            minor_version,
            game_id,
            compilation_time,
            src_file_name,
            user_name,
            machine_name,
            string_table,
            debug_info,
            user_flags,
            objects,
        })
    }
}
