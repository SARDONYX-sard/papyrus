// SPDX-FileCopyrightText: (C) 2025 russo-2025
// SPDX-License-Identifier: MIT
//! Writing `.pex` files.

use crate::{Error, pex::*};

/// Appends big-endian values to a buffer.
#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn len(&mut self, len: usize) -> Result<(), Error> {
        let len = u16::try_from(len).map_err(|_| Error::OverFlowVecLen { len })?;
        self.u16(len);
        Ok(())
    }

    fn string(&mut self, value: &PexStr<'_>) -> Result<(), Error> {
        self.len(value.as_bytes().len())?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn id(&mut self, value: StringId) {
        self.u16(value.0);
    }

    fn value(&mut self, value: &VariableValue) {
        match *value {
            VariableValue::Null => self.u8(0),
            VariableValue::Identifier(id) => {
                self.u8(1);
                self.id(id);
            }
            VariableValue::String(id) => {
                self.u8(2);
                self.id(id);
            }
            VariableValue::Integer(v) => {
                self.u8(3);
                self.i32(v);
            }
            VariableValue::Float(v) => {
                self.u8(4);
                self.u32(v.to_bits());
            }
            VariableValue::Boolean(v) => {
                self.u8(5);
                self.u8(u8::from(v));
            }
        }
    }

    fn variable_types(&mut self, vars: &[VariableType]) -> Result<(), Error> {
        self.len(vars.len())?;
        for v in vars {
            self.id(v.name);
            self.id(v.ty);
        }
        Ok(())
    }

    fn function_info(&mut self, info: &FunctionInfo) -> Result<(), Error> {
        self.id(info.return_type);
        self.id(info.docstring);
        self.u32(info.user_flags);
        self.u8(info.flags.bits());
        self.variable_types(&info.params)?;
        self.variable_types(&info.locals)?;
        self.len(info.instructions.len())?;
        for instruction in &info.instructions {
            self.u8(instruction.opcode as u8);
            for arg in &instruction.args {
                self.value(arg);
            }
        }
        Ok(())
    }

    fn property(&mut self, property: &Property) -> Result<(), Error> {
        self.id(property.name);
        self.id(property.ty);
        self.id(property.docstring);
        self.u32(property.user_flags);
        self.u8(property.flags.bits());
        if property.is_auto_var() {
            let name = property.auto_var_name.ok_or_else(|| Error::MissingAutoVerInProperty {
                name: property.name.to_string(),
            })?;
            self.id(name);
        } else {
            if let Some(handler) = &property.read_handler {
                self.function_info(handler)?;
            }
            if let Some(handler) = &property.write_handler {
                self.function_info(handler)?;
            }
        }
        Ok(())
    }

    fn object(&mut self, object: &Object) -> Result<(), Error> {
        self.id(object.name);
        // The size counts itself and everything after it in the object.
        let size_offset = self.bytes.len();
        self.u32(0);
        self.id(object.parent_class_name);
        self.id(object.docstring);
        self.u32(object.user_flags.bits());
        self.id(object.auto_state_name);
        self.len(object.variables.len())?;
        for v in &object.variables {
            self.id(v.name);
            self.id(v.type_name);
            self.u32(v.user_flags);
            self.value(&v.value);
        }
        self.len(object.properties.len())?;
        for p in &object.properties {
            self.property(p)?;
        }
        self.len(object.states.len())?;
        for state in &object.states {
            self.id(state.name);
            self.len(state.functions.len())?;
            for f in &state.functions {
                self.id(f.name);
                self.function_info(&f.info)?;
            }
        }
        let size = (self.bytes.len() - size_offset) as u32;
        self.bytes[size_offset..size_offset + 4].copy_from_slice(&size.to_be_bytes());
        Ok(())
    }
}

impl PexFile<'_> {
    /// Writes the file.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut w = Writer::default();
        w.u32(self.magic_number);
        w.u8(self.major_version);
        w.u8(self.minor_version);
        w.u16(self.game_id.to_u16());
        w.i64(self.compilation_time);
        w.string(&self.src_file_name)?;
        w.string(&self.user_name)?;
        w.string(&self.machine_name)?;
        w.len(self.string_table.len())?;
        for s in &self.string_table {
            w.string(s)?;
        }
        match &self.debug_info {
            Some(debug) => {
                w.u8(1);
                w.i64(debug.modification_time);
                w.len(debug.functions.len())?;
                for f in &debug.functions {
                    w.id(f.object_name);
                    w.id(f.state_name);
                    w.id(f.function_name);
                    w.u8(f.function_type as u8);
                    w.len(f.instruction_line_numbers.len())?;
                    for &line in &f.instruction_line_numbers {
                        w.u16(line);
                    }
                }
            }
            None => w.u8(0),
        }
        w.len(self.user_flags.len())?;
        for flag in &self.user_flags {
            w.id(flag.name);
            w.u8(flag.flag_index);
        }
        w.len(self.objects.len())?;
        for object in &self.objects {
            w.object(object)?;
        }
        Ok(w.bytes)
    }
}
