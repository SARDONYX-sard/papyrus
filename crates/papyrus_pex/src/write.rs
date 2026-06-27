use crate::{Error, pex::*};

pub struct Writer {
    bytes: Vec<u8>,
}

impl Default for Writer {
    fn default() -> Self {
        Self {
            bytes: Vec::with_capacity(2048),
        }
    }
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    #[inline]
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    #[inline]
    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    #[inline]
    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    #[inline]
    fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    #[inline]
    fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    #[inline]
    fn f32(&mut self, value: f32) {
        self.u32(value.to_bits());
    }

    #[inline]
    fn string(&mut self, value: &str) -> Result<(), Error> {
        self.write_vec_len(value.len())?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    #[inline]
    fn string_id(&mut self, value: StringId) {
        self.u16(value.0);
    }

    fn write_variable_type(&mut self, variable: &VariableType) {
        self.string_id(variable.name);
        self.string_id(variable.ty);
    }

    fn write_variable_value(&mut self, value: &VariableValue) {
        match value {
            VariableValue::Null => {
                self.u8(0);
            }

            VariableValue::Identifier(id) => {
                self.u8(1);
                self.string_id(*id);
            }

            VariableValue::String(id) => {
                self.u8(2);
                self.string_id(*id);
            }

            VariableValue::Integer(v) => {
                self.u8(3);
                self.i32(*v);
            }

            VariableValue::Float(v) => {
                self.u8(4);
                self.f32(*v);
            }

            VariableValue::Boolean(v) => {
                self.u8(5);
                self.u8(u8::from(*v));
            }
        }
    }

    fn write_variable(&mut self, variable: &Variable) {
        self.string_id(variable.name);
        self.string_id(variable.type_name);
        self.u32(variable.user_flags);
        self.write_variable_value(&variable.value);
    }

    fn write_instruction(&mut self, instruction: &Instruction) {
        self.u8(instruction.opcode as u8);

        for arg in &instruction.args {
            self.write_variable_value(arg);
        }
    }

    fn write_function_info(&mut self, info: &FunctionInfo) -> Result<(), Error> {
        self.string_id(info.return_type);
        self.string_id(info.docstring);

        self.u32(info.user_flags);
        self.u8(info.flags.bits());

        self.write_vec_len(info.params.len())?;
        for param in &info.params {
            self.write_variable_type(param);
        }

        self.write_vec_len(info.locals.len())?;
        for local in &info.locals {
            self.write_variable_type(local);
        }

        self.write_vec_len(info.instructions.len())?;
        for instruction in &info.instructions {
            self.write_instruction(instruction);
        }

        Ok(())
    }

    fn write_function(&mut self, function: &Function) -> Result<(), Error> {
        self.string_id(function.name);
        self.write_function_info(&function.info)
    }

    fn write_property(&mut self, property: &Property) -> Result<(), Error> {
        self.string_id(property.name);
        self.string_id(property.ty);
        self.string_id(property.docstring);

        self.u32(property.user_flags);
        self.u8(property.flags.bits());

        if property.flags.contains(PropertyFlags::AUTO_VAR) {
            let Some(auto_var_name) = property.auto_var_name else {
                return Err(Error::MissingAutoVerInProperty {
                    name: property.name.to_string(),
                });
            };
            self.string_id(auto_var_name);
        } else {
            if let Some(handler) = &property.read_handler {
                self.write_function_info(handler)?;
            }

            if let Some(handler) = &property.write_handler {
                self.write_function_info(handler)?;
            }
        }

        Ok(())
    }

    fn write_state(&mut self, state: &State) -> Result<(), Error> {
        self.string_id(state.name);

        self.write_vec_len(state.functions.len())?;

        for function in &state.functions {
            self.write_function(function)?;
        }
        Ok(())
    }

    fn write_object(&mut self, object: &Object) -> Result<(), Error> {
        self.string_id(object.name);

        let size_offset = self.bytes.len();

        self.u32(0);

        self.string_id(object.parent_class_name);
        self.string_id(object.docstring);

        self.u32(object.user_flags.bits());

        self.string_id(object.auto_state_name);

        self.write_vec_len(object.variables.len())?;
        for variable in &object.variables {
            self.write_variable(variable);
        }

        self.write_vec_len(object.properties.len())?;
        for property in &object.properties {
            self.write_property(property)?;
        }

        self.write_vec_len(object.states.len())?;
        for state in &object.states {
            self.write_state(state)?;
        }

        let size = (self.bytes.len() - size_offset) as u32;

        self.bytes[size_offset..size_offset + 4].copy_from_slice(&size.to_be_bytes());
        Ok(())
    }

    fn write_vec_len(&mut self, len: usize) -> Result<(), Error> {
        self.u16({
            if len > (u16::MAX as usize) {
                return Err(Error::OverFlowVecLen { len });
            } else {
                len as u16
            }
        });
        Ok(())
    }
}

impl<'a> PexFile<'a> {
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut writer = Writer::new();

        writer.u32(self.magic_number);

        writer.u8(self.major_version);
        writer.u8(self.minor_version);

        writer.u16(self.game_id.to_u16());

        writer.i64(self.compilation_time);

        writer.string(self.src_file_name)?;
        writer.string(self.user_name)?;
        writer.string(self.machine_name)?;

        writer.write_vec_len(self.string_table.len())?;
        for string in &self.string_table {
            writer.string(string)?;
        }

        if let Some(debug_info) = &self.debug_info {
            writer.u8(1); // has_debug_info

            writer.i64(debug_info.modification_time);

            writer.write_vec_len(debug_info.functions.len())?;
            for function in &debug_info.functions {
                writer.string_id(function.object_name);
                writer.string_id(function.state_name);
                writer.string_id(function.function_name);

                writer.u8(match function.function_type {
                    DebugFunctionType::Method => 0,
                    DebugFunctionType::Getter => 1,
                    DebugFunctionType::Setter => 2,
                });

                writer.write_vec_len(function.instruction_line_numbers.len())?;
                for line in &function.instruction_line_numbers {
                    writer.u16(*line);
                }
            }
        } else {
            writer.u8(0); // has_debug_info
        }

        writer.write_vec_len(self.user_flags.len())?;
        for flag in &self.user_flags {
            writer.string_id(flag.name);
            writer.u8(flag.flag_index);
        }

        writer.write_vec_len(self.objects.len())?;
        for object in &self.objects {
            writer.write_object(object)?;
        }

        Ok(writer.into_bytes())
    }
}
