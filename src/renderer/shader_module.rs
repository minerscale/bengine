use std::{ops::Deref, rc::Rc};

use ash::vk;
use log::info;

pub struct SpecializationInfo<'a> {
    info: vk::SpecializationInfo<'a>,
}

impl<'a> SpecializationInfo<'a> {
    pub fn new(info: &'a [vk::SpecializationMapEntry], data: &'a [u8]) -> Self {
        Self {
            info: vk::SpecializationInfo::default()
                .map_entries(info)
                .data(data),
        }
    }
}

impl<'a> Deref for SpecializationInfo<'a> {
    type Target = vk::SpecializationInfo<'a>;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

pub struct ShaderModule<'a> {
    device: Rc<ash::Device>,
    shader: vk::ShaderModule,
    pub stage: vk::ShaderStageFlags,
    pub specialization_info: Option<SpecializationInfo<'a>>,
}

macro_rules! spv {
    ($device:expr, $filename:literal, $stage:expr, $specialization:expr) => {{
        crate::renderer::shader_module::ShaderModule::new(
            $device,
            unsafe {
                let mut code = std::io::Cursor::new(
                    &(include_bytes!(concat!(env!("OUT_DIR"), "/", $filename, ".spv")))[..],
                );

                $device
                    .create_shader_module(
                        &vk::ShaderModuleCreateInfo::default().code(
                            &ash::util::read_spv(&mut code).expect("failed to read {$filename}"),
                        ),
                        None,
                    )
                    .expect("failed to build shader module!")
            },
            $stage,
            $specialization,
        )
    }};
}
pub(crate) use spv;

impl<'a> ShaderModule<'a> {
    pub fn new(
        device: Rc<ash::Device>,
        shader: vk::ShaderModule,
        stage: vk::ShaderStageFlags,
        specialization_info: Option<SpecializationInfo<'a>>,
    ) -> Self {
        ShaderModule {
            shader,
            device,
            stage,
            specialization_info,
        }
    }
}

impl<'a> Deref for ShaderModule<'a> {
    type Target = vk::ShaderModule;

    fn deref(&self) -> &Self::Target {
        &self.shader
    }
}

impl<'a> Drop for ShaderModule<'a> {
    fn drop(&mut self) {
        info!("dropped shader module");
        unsafe {
            self.device.destroy_shader_module(self.shader, None);
        }
    }
}
