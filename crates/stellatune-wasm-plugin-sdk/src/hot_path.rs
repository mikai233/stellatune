use crate::common::{BufferLayout, CoreModuleSpec, HotPathRole, SampleFormat};
use crate::error::{SdkError, SdkResult};

pub const HOT_PATH_ABI_VERSION_V1: u32 = 1;

pub const DEFAULT_MEMORY_EXPORT: &str = "memory";
pub const DEFAULT_INIT_EXPORT: &str = "st_hot_init";
pub const DEFAULT_PROCESS_EXPORT: &str = "st_hot_process";
pub const DEFAULT_RESET_EXPORT: &str = "st_hot_reset";
pub const DEFAULT_DROP_EXPORT: &str = "st_hot_drop";

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotInitArgs {
    pub abi_version: u32,
    pub role: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: u16,
    pub max_frames: u32,
    pub in_offset: u32,
    pub out_offset: u32,
    pub buffer_bytes: u32,
    pub flags: u32,
    pub reserved0: u32,
    pub reserved1: u32,
}

impl HotInitArgs {
    pub fn role_value(role: HotPathRole) -> u32 {
        match role {
            HotPathRole::DspTransform => 1,
            HotPathRole::OutputSink => 2,
        }
    }

    pub fn sample_format_value(sample_format: SampleFormat) -> u16 {
        match sample_format {
            SampleFormat::F32Le => 1,
            SampleFormat::I16Le => 2,
            SampleFormat::I32Le => 3,
        }
    }
}

pub const HOT_INIT_ARGS_SIZE: usize = core::mem::size_of::<HotInitArgs>();

#[derive(Debug, Clone)]
pub struct CoreModuleSpecBuilder {
    spec: CoreModuleSpec,
}

impl CoreModuleSpecBuilder {
    pub fn new(role: HotPathRole, wasm_rel_path: impl Into<String>, buffer: BufferLayout) -> Self {
        Self {
            spec: CoreModuleSpec {
                role,
                wasm_rel_path: wasm_rel_path.into(),
                abi_version: HOT_PATH_ABI_VERSION_V1,
                memory_export: DEFAULT_MEMORY_EXPORT.to_string(),
                init_export: DEFAULT_INIT_EXPORT.to_string(),
                process_export: DEFAULT_PROCESS_EXPORT.to_string(),
                reset_export: Some(DEFAULT_RESET_EXPORT.to_string()),
                drop_export: Some(DEFAULT_DROP_EXPORT.to_string()),
                buffer,
            },
        }
    }

    pub fn abi_version(mut self, abi_version: u32) -> Self {
        self.spec.abi_version = abi_version;
        self
    }

    pub fn memory_export(mut self, name: impl Into<String>) -> Self {
        self.spec.memory_export = name.into();
        self
    }

    pub fn init_export(mut self, name: impl Into<String>) -> Self {
        self.spec.init_export = name.into();
        self
    }

    pub fn process_export(mut self, name: impl Into<String>) -> Self {
        self.spec.process_export = name.into();
        self
    }

    pub fn reset_export(mut self, name: Option<impl Into<String>>) -> Self {
        self.spec.reset_export = name.map(Into::into);
        self
    }

    pub fn drop_export(mut self, name: Option<impl Into<String>>) -> Self {
        self.spec.drop_export = name.map(Into::into);
        self
    }

    pub fn build(self) -> SdkResult<CoreModuleSpec> {
        validate_core_module_spec(&self.spec)?;
        Ok(self.spec)
    }
}

pub fn validate_core_module_spec(spec: &CoreModuleSpec) -> SdkResult<()> {
    if spec.abi_version != HOT_PATH_ABI_VERSION_V1 {
        return Err(SdkError::invalid_arg(format!(
            "unsupported hot-path ABI version: {} (expect {})",
            spec.abi_version, HOT_PATH_ABI_VERSION_V1
        )));
    }
    if spec.wasm_rel_path.trim().is_empty() {
        return Err(SdkError::invalid_arg("wasm-rel-path is empty"));
    }
    if spec.memory_export.trim().is_empty() {
        return Err(SdkError::invalid_arg("memory-export is empty"));
    }
    if spec.init_export.trim().is_empty() {
        return Err(SdkError::invalid_arg("init-export is empty"));
    }
    if spec.process_export.trim().is_empty() {
        return Err(SdkError::invalid_arg("process-export is empty"));
    }
    validate_buffer_layout(spec.role, &spec.buffer)
}

pub fn validate_buffer_layout(role: HotPathRole, buffer: &BufferLayout) -> SdkResult<()> {
    if buffer.max_frames == 0 {
        return Err(SdkError::invalid_arg("buffer.max-frames must be > 0"));
    }
    if buffer.channels == 0 {
        return Err(SdkError::invalid_arg("buffer.channels must be > 0"));
    }
    if buffer.in_offset == 0 && buffer.interleaved {
        return Err(SdkError::invalid_arg(
            "buffer.in-offset should be non-zero for interleaved hot-path",
        ));
    }
    if matches!(role, HotPathRole::DspTransform) && buffer.out_offset.is_none() {
        return Err(SdkError::invalid_arg(
            "buffer.out-offset is required for dsp-transform role",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CoreModuleSpecBuilder, HOT_INIT_ARGS_SIZE, HotInitArgs, validate_buffer_layout,
        validate_core_module_spec,
    };
    use crate::common::{BufferLayout, HotPathRole, SampleFormat};

    #[test]
    fn hot_init_args_size_is_stable() {
        assert_eq!(HOT_INIT_ARGS_SIZE, 44);
        assert_eq!(core::mem::align_of::<HotInitArgs>(), 4);
    }

    #[test]
    fn validate_rejects_missing_dsp_out_offset() {
        let layout = BufferLayout {
            in_offset: 4096,
            out_offset: None,
            max_frames: 1024,
            channels: 2,
            sample_format: SampleFormat::F32Le,
            interleaved: true,
        };
        assert!(validate_buffer_layout(HotPathRole::DspTransform, &layout).is_err());
    }

    #[test]
    fn builder_creates_valid_spec() {
        let layout = BufferLayout {
            in_offset: 4096,
            out_offset: Some(8192),
            max_frames: 1024,
            channels: 2,
            sample_format: SampleFormat::F32Le,
            interleaved: true,
        };
        let spec =
            CoreModuleSpecBuilder::new(HotPathRole::DspTransform, "wasm/hot_dsp.wasm", layout)
                .build()
                .expect("builder should produce valid spec");

        validate_core_module_spec(&spec).expect("spec should validate");
    }
}
