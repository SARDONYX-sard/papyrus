use crate::{Error, binary::Reader, pex::*};

#[cfg(feature = "trace-layout")]
macro_rules! trace_struct {
    ($reader:expr, $label:expr, $expr:expr) => {{
        let start = $reader.offset();

        let value = $expr;

        let end = $reader.offset();

        $reader.annotate(start, end - start, $label);

        value
    }};
}

#[cfg(not(feature = "trace-layout"))]
macro_rules! trace_struct {
    ($reader:expr, $label:expr, $expr:expr) => {{ $expr }};
}

#[cfg(feature = "trace-layout")]
macro_rules! trace_field {
    ($reader:expr, $label:expr, $expr:expr) => {{
        let start = $reader.offset();
        let value = $expr;
        let end = $reader.offset();

        $reader.annotate(start, end - start, $label);

        value
    }};
}

#[cfg(not(feature = "trace-layout"))]
macro_rules! trace_field {
    ($reader:expr, $label:expr, $expr:expr) => {{ $expr }};
}

pub trait ReadPex: Sized {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error>;
}

fn read_vec<T: ReadPex>(reader: &mut Reader<'_>) -> Result<Vec<T>, Error> {
    let len = reader.u16()? as usize;

    let mut vec = Vec::with_capacity(len);
    for _ in 0..len {
        vec.push(T::read(reader)?);
    }

    Ok(vec)
}

fn read_vec_map<'a, T, F>(reader: &mut Reader<'a>, f: F) -> Result<Vec<T>, Error>
where
    T: 'a,
    F: Fn(&mut Reader<'a>) -> Result<T, Error>,
{
    let len = reader.u16()? as usize;

    let mut vec = Vec::with_capacity(len);
    for _ in 0..len {
        vec.push(f(reader)?);
    }

    Ok(vec)
}

impl ReadPex for VariableType {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            name: reader.string_id()?,
            ty: reader.string_id()?,
        })
    }
}

impl ReadPex for Variable {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            name: reader.string_id()?,
            type_name: reader.string_id()?,
            user_flags: reader.u32()?,
            value: VariableValue::read(reader)?,
        })
    }
}

impl ReadPex for VariableValue {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let offset = reader.offset();

        Ok(match reader.u8()? {
            0 => Self::Null,
            1 => Self::Identifier(reader.string_id()?),
            2 => Self::String(reader.string_id()?),
            3 => Self::Integer(reader.i32()?),
            4 => Self::Float(reader.f32()?),
            5 => Self::Boolean(reader.u8()? != 0),
            invalid => {
                return Err(Error::InvalidVariableValue {
                    offset,
                    value: invalid,
                });
            }
        })
    }
}

impl ReadPex for UserFlag {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(trace_struct!(
            reader,
            "UserFlag",
            Self {
                name: trace_field!(reader, "name", reader.string_id()?),
                flag_index: trace_field!(reader, "flag_index", reader.u8()?),
            }
        ))
    }
}

impl ReadPex for FunctionFlags {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let offset = reader.offset();
        let flags = reader.u8()?;
        FunctionFlags::from_bits(flags).ok_or(Error::InvalidFunctionFlags {
            offset,
            value: flags,
        })
    }
}

impl ReadPex for FunctionInfo {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(trace_struct!(
            reader,
            "FunctionInfo",
            Self {
                return_type: trace_field!(reader, "return_type", reader.string_id()?),
                docstring: trace_field!(reader, "docstring", reader.string_id()?),
                user_flags: trace_field!(reader, "user_flags", reader.u32()?),
                flags: trace_field!(reader, "flags", FunctionFlags::read(reader)?),
                params: trace_field!(reader, "params", read_vec(reader)?),
                locals: trace_field!(reader, "locals", read_vec(reader)?),
                instructions: trace_field!(reader, "instructions", read_vec(reader)?),
            }
        ))
    }
}

impl ReadPex for Function {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            name: reader.string_id()?,
            info: FunctionInfo::read(reader)?,
        })
    }
}

impl ReadPex for Instruction {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let opcode = OpCode::read(reader)?;

        let mut args = Vec::new();
        for _ in 0..opcode.argument_count() {
            args.push(VariableValue::read(reader)?);
        }

        if matches!(
            opcode,
            OpCode::CallMethod | OpCode::CallParent | OpCode::CallStatic
        ) {
            let arg = VariableValue::read(reader)?;
            let len = match arg {
                VariableValue::Integer(len) => Some(len),
                _ => None,
            };
            args.push(arg);

            if let Some(len) = len {
                for _ in 0..len {
                    args.push(VariableValue::read(reader)?);
                }
            }
        }

        Ok(Self { opcode, args })
    }
}

impl ReadPex for OpCode {
    #[inline]
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let offset = reader.offset();
        OpCode::try_from(reader.u8()?).map_err(|e| Error::InvalidOpCode { offset, source: e })
    }
}

impl ReadPex for State {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            name: reader.string_id()?,
            functions: read_vec(reader)?,
        })
    }
}

impl ReadPex for Object {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            name: reader.string_id()?,
            size: reader.u32()?,

            parent_class_name: reader.string_id()?,
            docstring: reader.string_id()?,

            user_flags: ObjectFlags::from_bits_retain(reader.u32()?),

            auto_state_name: reader.string_id()?,

            variables: read_vec(reader)?,
            properties: read_vec(reader)?,
            states: read_vec(reader)?,
        })
    }
}
impl ReadPex for Property {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let name = reader.string_id()?;
        let ty = reader.string_id()?;
        let docstring = reader.string_id()?;
        let user_flags = reader.u32()?;

        let flags = PropertyFlags::from_bits_retain(reader.u8()?);

        let (auto_var_name, read_handler, write_handler) =
            if flags.contains(PropertyFlags::AUTO_VAR) {
                (Some(reader.string_id()?), None, None)
            } else {
                (
                    None,
                    if flags.contains(PropertyFlags::READ) {
                        Some(FunctionInfo::read(reader)?)
                    } else {
                        None
                    },
                    if flags.contains(PropertyFlags::WRITE) {
                        Some(FunctionInfo::read(reader)?)
                    } else {
                        None
                    },
                )
            };

        Ok(Self {
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
}

impl ReadPex for DebugInfo {
    #[inline]
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            modification_time: reader.i64()?,
            functions: read_vec(reader)?,
        })
    }
}

impl ReadPex for DebugFunction {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            object_name: reader.string_id()?,
            state_name: reader.string_id()?,

            function_name: reader.string_id()?,
            function_type: DebugFunctionType::read(reader)?,

            instruction_line_numbers: read_vec_map(reader, |r| r.u16())?,
        })
    }
}

impl ReadPex for DebugFunctionType {
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let offset = reader.offset();
        Ok(match reader.u8()? {
            0 => Self::Method,
            1 => Self::Getter,
            2 => Self::Setter,
            invalid => {
                return Err(Error::InvalidDebugFunctionType {
                    offset,
                    value: invalid,
                });
            }
        })
    }
}

impl ReadPex for GameType {
    #[inline]
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(match reader.u16()? {
            1 => GameType::Skyrim,
            value => {
                return Err(Error::InvalidGameType {
                    offset: reader.offset(),
                    value,
                });
            }
        })
    }
}

impl<'a> PexFile<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Error> {
        Self::from_reader_impl(bytes, None::<&mut std::io::Sink>)
    }

    #[cfg(feature = "trace-layout")]
    pub fn from_bytes_with_annotations<W>(bytes: &'a [u8], writer: &mut W) -> Result<Self, Error>
    where
        W: std::io::Write,
    {
        Self::from_reader_impl(bytes, Some(writer))
    }

    fn from_reader_impl<W>(
        bytes: &'a [u8],
        #[allow(unused)] annotation_writer: Option<&mut W>,
    ) -> Result<Self, Error>
    where
        W: std::io::Write,
    {
        let mut reader = Reader::new(bytes);

        let magic_number = trace_field!(reader, "magic_number", reader.u32()?);
        if magic_number != LE_MAGIC_NUMBER {
            return Err(Error::InvalidMagicNumber {
                offset: 0,
                value: magic_number,
            });
        }

        let major_version = trace_field!(reader, "major_version", reader.u8()?);
        let minor_version = trace_field!(reader, "minor_version", reader.u8()?);
        let game_id = trace_field!(reader, "game_id", GameType::read(&mut reader)?);

        let compilation_time = trace_field!(reader, "compilation_time", reader.i64()?);
        let src_file_name = trace_field!(reader, "src_file_name", reader.string()?);
        let user_name = trace_field!(reader, "user_name", reader.string()?);
        let machine_name = trace_field!(reader, "machine_name", reader.string()?);

        {
            let string_table = trace_field!(
                reader,
                "string_table",
                read_vec_map(&mut reader, |reader| reader.string())?
            );
            reader.set_string_table(string_table);
        }

        let debug_info = {
            let has_debug_info = reader.u8()? != 0;
            has_debug_info.then_some(DebugInfo::read(&mut reader)?)
        };

        let user_flags = trace_field!(reader, "user_flags", read_vec(&mut reader)?);

        #[cfg(feature = "trace-layout")]
        if let Some(writer) = annotation_writer {
            render_annotations(writer, bytes, &reader.annotations)?;
        }

        let objects = trace_field!(reader, "objects", read_vec(&mut reader)?);

        Ok(Self {
            magic_number,
            major_version,
            minor_version,
            game_id,
            compilation_time,
            src_file_name,
            user_name,
            machine_name,
            string_table: reader.take_string_table(),
            debug_info,
            user_flags,
            objects,
        })
    }
}

#[cfg(feature = "trace-layout")]
pub fn render_annotations<W>(
    writer: &mut W,
    bytes: &[u8],
    annotations: &[crate::binary::Annotation],
) -> std::io::Result<()>
where
    W: std::io::Write,
{
    for line_base in (0..bytes.len()).step_by(16) {
        let line_end = (line_base + 16).min(bytes.len());

        write!(writer, "{line_base:08x}: ")?;

        for b in &bytes[line_base..line_end] {
            write!(writer, "{b:02x} ")?;
        }

        writeln!(writer)?;

        for ann in annotations {
            let ann_start = ann.offset;
            let ann_end = ann.offset + ann.size;

            let visible_start = ann_start.max(line_base);
            let visible_end = ann_end.min(line_end);

            if visible_start >= visible_end {
                continue;
            }

            let start_col = (visible_start - line_base) * 3;
            let width = (visible_end - visible_start) * 3;

            write!(writer, "{:10}", "")?;
            write!(writer, "{:start_col$}", "")?;

            write!(writer, "<")?;

            for _ in 1..width.max(2) {
                write!(writer, "-")?;
            }

            write!(writer, ">")?;

            if visible_start == ann_start {
                write!(writer, " {}", ann.label)?;
            }

            writeln!(writer)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_file() {
        #[rustfmt::skip]
        let path = {
            // let path = r"D:\GAME\ModOrganizer Skyrim SE\mods\Song-of-the-Sea\Scripts\SOTS_AttachPlayerAttackTracker.pex";
            let path = r"D:\GAME\ModOrganizer Skyrim SE\mods\Song-of-the-Sea\Scripts\SOTS_MannequinPicker.pex";
            // let path = "D:/Programming/cpp/fnis_aa/cxx/papyrus/prebuilt/fnis.pex";
            path
        };
        let bytes = std::fs::read(path).unwrap();
        let pex_file = PexFile::from_bytes(&bytes).unwrap();
        let actual_bytes = pex_file.to_bytes().unwrap();

        assert_eq!(actual_bytes, bytes);

        std::fs::write("../target/debug.log", format!("{pex_file:#?}")).unwrap();
    }
}
