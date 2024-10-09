use std::{ops::Deref, rc::Rc};

use ash::vk;
use log::info;

pub struct ShaderModule {
    shader: vk::ShaderModule,
    device: Rc<ash::Device>,
}

macro_rules! spv {
    ($device:expr, $filename:literal) => {{
        crate::shader_module::ShaderModule::new(
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
            $device,
        )
    }};
}
pub(crate) use spv;

impl ShaderModule {
    pub fn new(shader: vk::ShaderModule, device: Rc<ash::Device>) -> Self {
        ShaderModule { shader, device }
    }
}

impl Deref for ShaderModule {
    type Target = vk::ShaderModule;

    fn deref(&self) -> &Self::Target {
        &self.shader
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        info!("dropped shader module");
        unsafe {
            self.device.destroy_shader_module(self.shader, None);
        }
    }
}
