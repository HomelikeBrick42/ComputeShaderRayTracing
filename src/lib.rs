use cgmath::{Quaternion, Rotation3};
use eframe::egui;
use encase::{ArrayLength, ShaderSize, ShaderType, StorageBuffer, UniformBuffer};
use wgpu::util::DeviceExt;

#[derive(Clone, Copy)]
struct Camera {
    position: cgmath::Vector3<f32>,
    rotation: Quaternion<f32>,
    up_sky_color: cgmath::Vector3<f32>,
    down_sky_color: cgmath::Vector3<f32>,
    min_distance: f32,
    max_distance: f32,
}

#[derive(Clone, Copy, ShaderType)]
struct CameraUniform {
    position: cgmath::Vector3<f32>,
    forward: cgmath::Vector3<f32>,
    right: cgmath::Vector3<f32>,
    up: cgmath::Vector3<f32>,
    up_sky_color: cgmath::Vector3<f32>,
    down_sky_color: cgmath::Vector3<f32>,
    min_distance: f32,
    max_distance: f32,
}

#[derive(Clone, Copy, ShaderType)]
struct Sphere {
    position: cgmath::Vector3<f32>,
    radius: f32,
    color: cgmath::Vector3<f32>,
}

impl Default for Sphere {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0, 0.0).into(),
            radius: 1.0,
            color: (1.0, 1.0, 1.0).into(),
        }
    }
}

#[derive(Clone, ShaderType)]
struct SpheresBuffer {
    sphere_count: ArrayLength,
    #[size(runtime)]
    spheres: Vec<Sphere>,
}

impl From<Camera> for CameraUniform {
    fn from(camera: Camera) -> Self {
        let forward = camera.rotation * cgmath::vec3(0.0, 0.0, 1.0);
        let right = camera.rotation * cgmath::vec3(1.0, 0.0, 0.0);
        let up = camera.rotation * cgmath::vec3(0.0, 1.0, 0.0);
        Self {
            position: camera.position,
            forward,
            right,
            up,
            up_sky_color: camera.up_sky_color,
            down_sky_color: camera.down_sky_color,
            min_distance: camera.min_distance,
            max_distance: camera.max_distance,
        }
    }
}

pub struct App {
    last_frame_time: std::time::Instant,
    fixed_update_time: f64, // change this to std::time::Duration at some point
    last_frame_update_duration: std::time::Duration,
    last_fixed_update_duration: std::time::Duration,
    texture_size: (usize, usize),
    texture_bind_group: wgpu::BindGroup,
    texture_id: egui::TextureId,
    pipeline: wgpu::ComputePipeline,
    camera: Camera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    spheres_storage: SpheresBuffer,
    spheres_buffer: wgpu::Buffer,
    spheres_bind_group: wgpu::BindGroup,
    spheres_buffer_size: usize,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let render_state = cc.wgpu_render_state.as_ref().unwrap();

        let (width, height) = (1usize, 1usize);
        let texture_size = wgpu::Extent3d {
            width: width as _,
            height: height as _,
            depth_or_array_layers: 1,
        };

        let texture = render_state
            .device
            .create_texture(&wgpu::TextureDescriptor {
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
                label: Some("texture"),
                view_formats: &[],
            });

        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &texture.create_view(&wgpu::TextureViewDescriptor {
                ..Default::default()
            }),
            wgpu::FilterMode::Linear,
        );

        let shader = render_state
            .device
            .create_shader_module(wgpu::include_wgsl!("./shader.wgsl"));

        let pipeline =
            render_state
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Pipeline"),
                    layout: None,
                    module: &shader,
                    entry_point: "main",
                });

        let texture_bind_group =
            render_state
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Texture bind group"),
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    }],
                });

        let camera = Camera {
            position: (0.0, 0.0, -3.0).into(),
            rotation: Quaternion::from_axis_angle((0.0, 0.0, 1.0).into(), cgmath::Deg(0.0)),
            up_sky_color: (1.0, 1.0, 1.0).into(),
            down_sky_color: (0.5, 0.7, 1.0).into(),
            min_distance: 0.001,
            max_distance: 1000.0,
        };

        let camera_buffer = {
            let camera_uniform: CameraUniform = camera.into();
            let mut buffer =
                UniformBuffer::new([0u8; <CameraUniform as ShaderSize>::SHADER_SIZE.get() as _]);
            buffer.write(&camera_uniform).unwrap();
            render_state
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera Buffer"),
                    contents: &buffer.into_inner(),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                })
        };

        let camera_bind_group = render_state
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &pipeline.get_bind_group_layout(1),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
                label: Some("camera_bind_group"),
            });

        let spheres_storage = SpheresBuffer {
            sphere_count: ArrayLength::default(),
            spheres: vec![Sphere::default()],
        };

        let (spheres_buffer, spheres_buffer_size) = {
            let mut buffer =
                StorageBuffer::new(Vec::with_capacity(spheres_storage.size().get() as _));
            buffer.write(&spheres_storage).unwrap();
            let buffer = buffer.into_inner();
            (
                render_state
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Sphere Buffer"),
                        contents: &buffer,
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    }),
                buffer.len(),
            )
        };

        let spheres_bind_group =
            render_state
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &pipeline.get_bind_group_layout(2),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: spheres_buffer.as_entire_binding(),
                    }],
                    label: Some("spheres_bind_group"),
                });

        Self {
            last_frame_time: std::time::Instant::now(),
            fixed_update_time: 0.0,
            last_frame_update_duration: std::time::Duration::ZERO,
            last_fixed_update_duration: std::time::Duration::ZERO,
            texture_size: (width, height),
            texture_bind_group,
            texture_id,
            pipeline,
            camera,
            camera_buffer,
            camera_bind_group,
            spheres_storage,
            spheres_buffer,
            spheres_bind_group,
            spheres_buffer_size,
        }
    }

    const FIXED_UPDATE_TIMESTEP: f64 = 1.0 / 60.0;

    fn render(
        &mut self,
        _ts: f64,
        render_state: &egui_wgpu::RenderState,
        size @ (width, height): (usize, usize),
    ) {
        let start_frame_time = std::time::Instant::now();

        if self.texture_size != size && width != 0 && height != 0 {
            let mut renderer = render_state.renderer.write();
            renderer.free_texture(&self.texture_id);

            let texture_size = wgpu::Extent3d {
                width: width as _,
                height: height as _,
                depth_or_array_layers: 1,
            };

            let texture = render_state
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    size: texture_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::STORAGE_BINDING,
                    label: Some("texture"),
                    view_formats: &[],
                });

            self.texture_id = renderer.register_native_texture(
                &render_state.device,
                &texture.create_view(&wgpu::TextureViewDescriptor {
                    ..Default::default()
                }),
                wgpu::FilterMode::Linear,
            );

            self.texture_bind_group =
                render_state
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Texture bind group"),
                        layout: &self.pipeline.get_bind_group_layout(0),
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                            ),
                        }],
                    });

            self.texture_size = size;
        }

        // Update camera uniform
        {
            let camera_uniform: CameraUniform = self.camera.into();
            let mut buffer =
                UniformBuffer::new([0u8; <CameraUniform as ShaderSize>::SHADER_SIZE.get() as _]);
            buffer.write(&camera_uniform).unwrap();
            render_state
                .queue
                .write_buffer(&self.camera_buffer, 0, &buffer.into_inner());
        }

        // Update spheres buffer
        {
            let mut buffer =
                StorageBuffer::new(Vec::with_capacity(self.spheres_storage.size().get() as _));
            buffer.write(&self.spheres_storage).unwrap();
            let buffer = buffer.into_inner();
            if self.spheres_buffer_size < buffer.len() {
                self.spheres_buffer =
                    render_state
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Sphere Buffer"),
                            contents: &buffer,
                            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        });

                self.spheres_bind_group =
                    render_state
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &self.pipeline.get_bind_group_layout(2),
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: self.spheres_buffer.as_entire_binding(),
                            }],
                            label: Some("spheres_bind_group"),
                        });

                self.spheres_buffer_size = buffer.len();
            } else {
                render_state
                    .queue
                    .write_buffer(&self.spheres_buffer, 0, &buffer);
            }
        }

        let mut encoder = render_state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let workgroup_size = (16, 16);
            let (dispatch_with, dispatch_height) = (
                (self.texture_size.0 + workgroup_size.0 - 1) / workgroup_size.0,
                (self.texture_size.1 + workgroup_size.1 - 1) / workgroup_size.1,
            );
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute pass"),
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &self.texture_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.spheres_bind_group, &[]);
            compute_pass.dispatch_workgroups(dispatch_with as _, dispatch_height as _, 1);
        }
        let submission_index = render_state.queue.submit([encoder.finish()]);

        // this is slow but its just so the timings are a bit more accurate
        render_state
            .device
            .poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));

        self.last_frame_update_duration = start_frame_time.elapsed();
    }

    fn fixed_update(&mut self) {
        let start_fixed_update_time = std::time::Instant::now();

        // TODO: update stuff

        self.last_fixed_update_duration = start_fixed_update_time.elapsed();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let start_time = std::time::Instant::now();
        let dt = start_time.duration_since(self.last_frame_time);
        let ts = dt.as_secs_f64();

        self.fixed_update_time += ts;
        while self.fixed_update_time >= Self::FIXED_UPDATE_TIMESTEP {
            self.fixed_update();
            self.fixed_update_time -= Self::FIXED_UPDATE_TIMESTEP;
        }

        egui::SidePanel::left("Counting").show(ctx, |ui| {
            ui.label(format!("FPS: {:.3}", 1.0 / ts));
            ui.label(format!(
                "Render time: {:.3}ms",
                self.last_frame_update_duration.as_secs_f64() * 1000.0
            ));
            ui.label(format!(
                "Fixed update time: {:.3}ms",
                self.last_fixed_update_duration.as_secs_f64() * 1000.0
            ));

            ui.horizontal(|ui| {
                ui.label("Up Sky Color:");
                let mut up_sky_color = self.camera.up_sky_color.into();
                egui::color_picker::color_edit_button_rgb(ui, &mut up_sky_color);
                self.camera.up_sky_color = up_sky_color.into();
            });
            ui.horizontal(|ui| {
                ui.label("Down Sky Color:");
                let mut down_sky_color = self.camera.down_sky_color.into();
                egui::color_picker::color_edit_button_rgb(ui, &mut down_sky_color);
                self.camera.down_sky_color = down_sky_color.into();
            });
            ui.horizontal(|ui| {
                ui.label("Min Distance:");
                ui.add(egui::DragValue::new(&mut self.camera.min_distance).speed(0.001));
                self.camera.min_distance = self.camera.min_distance.max(0.0001);
            });
            ui.horizontal(|ui| {
                ui.label("Max Distance:");
                ui.add(egui::DragValue::new(&mut self.camera.max_distance).speed(1.0));
                self.camera.max_distance = self.camera.max_distance.max(0.0);
            });

            ui.collapsing("Spheres", |ui| {
                if ui.button("Add Sphere").clicked() {
                    self.spheres_storage.spheres.push(Sphere::default());
                }
                let mut i = 0;
                while i < self.spheres_storage.spheres.len() {
                    let sphere = &mut self.spheres_storage.spheres[i as usize];
                    let mut to_remove = false;
                    ui.collapsing(format!("Sphere {i}"), |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Position:");
                            ui.add(
                                egui::DragValue::new(&mut sphere.position.x)
                                    .prefix("x: ")
                                    .speed(0.1),
                            );
                            ui.add(
                                egui::DragValue::new(&mut sphere.position.y)
                                    .prefix("y: ")
                                    .speed(0.1),
                            );
                            ui.add(
                                egui::DragValue::new(&mut sphere.position.z)
                                    .prefix("z: ")
                                    .speed(0.1),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            ui.add(egui::DragValue::new(&mut sphere.radius).speed(0.1));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Color:");
                            let mut color = sphere.color.into();
                            egui::color_picker::color_edit_button_rgb(ui, &mut color);
                            sphere.color = color.into();
                        });
                        if ui.button("Delete").clicked() {
                            to_remove = true;
                        }
                    });
                    if to_remove {
                        self.spheres_storage.spheres.remove(i as _);
                    } else {
                        i += 1;
                    }
                }
            });

            ui.allocate_space(ui.available_size());
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let size = ui.available_size();
                self.render(
                    ts,
                    frame.wgpu_render_state().unwrap(),
                    (size.x as _, size.y as _),
                );
                ui.image(self.texture_id, size);
            });

        if !ctx.wants_pointer_input() {
            ctx.input(|i| {
                if i.pointer.secondary_down() {
                    let rotation_horizontal = cgmath::Quaternion::from_angle_y(cgmath::Deg(
                        i.pointer.velocity().x * ts as f32,
                    ));
                    let rotation_vertical = cgmath::Quaternion::from_angle_x(cgmath::Deg(
                        i.pointer.velocity().y * ts as f32,
                    ));
                    self.camera.rotation = self.camera.rotation * rotation_horizontal;
                    self.camera.rotation = self.camera.rotation * rotation_vertical;
                }
            });
        }

        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                let rotation_horizontal = cgmath::Quaternion::from_angle_y(cgmath::Deg(
                    if i.key_down(egui::Key::ArrowLeft) {
                        -90.0 * ts as f32
                    } else if i.key_down(egui::Key::ArrowRight) {
                        90.0 * ts as f32
                    } else {
                        0.0
                    },
                ));
                let rotation_vertical = cgmath::Quaternion::from_angle_x(cgmath::Deg(
                    if i.key_down(egui::Key::ArrowUp) {
                        -90.0 * ts as f32
                    } else if i.key_down(egui::Key::ArrowDown) {
                        90.0 * ts as f32
                    } else {
                        0.0
                    },
                ));
                let rotation_roll =
                    cgmath::Quaternion::from_angle_z(cgmath::Deg(if i.key_down(egui::Key::Q) {
                        90.0 * ts as f32
                    } else if i.key_down(egui::Key::E) {
                        -90.0 * ts as f32
                    } else {
                        0.0
                    }));
                self.camera.rotation = self.camera.rotation * rotation_horizontal;
                self.camera.rotation = self.camera.rotation * rotation_vertical;
                self.camera.rotation = self.camera.rotation * rotation_roll;

                const CAMERA_SPEED: f32 = 2.0;

                let forward = self.camera.rotation * cgmath::vec3(0.0, 0.0, 1.0);
                let right = self.camera.rotation * cgmath::vec3(1.0, 0.0, 0.0);
                let up = self.camera.rotation * cgmath::vec3(0.0, 1.0, 0.0);

                if i.key_down(egui::Key::W) {
                    self.camera.position += CAMERA_SPEED * forward * ts as f32;
                }
                if i.key_down(egui::Key::S) {
                    self.camera.position -= CAMERA_SPEED * forward * ts as f32;
                }
                if i.key_down(egui::Key::A) {
                    self.camera.position -= CAMERA_SPEED * right * ts as f32;
                }
                if i.key_down(egui::Key::D) {
                    self.camera.position += CAMERA_SPEED * right * ts as f32;
                }
                if i.modifiers.ctrl {
                    self.camera.position -= CAMERA_SPEED * up * ts as f32;
                }
                if i.key_down(egui::Key::Space) {
                    self.camera.position += CAMERA_SPEED * up * ts as f32;
                }
            });
        }

        self.last_frame_time = start_time;
        ctx.request_repaint();
    }
}
