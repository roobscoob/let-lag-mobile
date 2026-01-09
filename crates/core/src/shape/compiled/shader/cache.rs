use std::{borrow::Cow, collections::HashMap};

pub struct ShaderCache {
    pub cache: HashMap<u64, wgpu::ShaderModule>,
}

impl ShaderCache {
    pub fn new() -> Self {
        ShaderCache {
            cache: HashMap::new(),
        }
    }

    pub fn get_or_create(
        &mut self,
        hash: u64,
        module: naga::Module,
        device: &wgpu::Device,
    ) -> wgpu::ShaderModule {
        if let Some(module) = self.cache.get(&hash) {
            return module.clone();
        }

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shape Shader Module"),
            source: wgpu::ShaderSource::Naga(Cow::Owned(module)),
        });

        self.cache.insert(hash, shader_module.clone());

        shader_module
    }
}
