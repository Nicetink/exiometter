use crate::audio_engine::{AudioEngine, DeviceInfo, AppAudioInfo};
use eframe::egui;
use std::sync::{Arc, Mutex};

pub struct ExIOMeterApp {
    audio_engine: Arc<Mutex<AudioEngine>>,
    available_devices: Vec<DeviceInfo>,
    selected_device_idx: Option<usize>,
    is_running: bool,
    error_message: Option<String>,
    connected: bool,
    
    // Settings
    dark_theme: bool,
    show_app_mixer: bool,
    show_about: bool,
    about_tab: usize,
    
    // App mixer data
    audio_apps: Vec<AppAudioInfo>,
    app_volumes: std::collections::HashMap<u32, f32>,
    app_mutes: std::collections::HashMap<u32, bool>,
    
    // Auto-update
    last_app_update: std::time::Instant,
    last_device_update: std::time::Instant,
    
    // Cached icon texture
    icon_texture: Option<egui::TextureHandle>,
}

impl ExIOMeterApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut audio_engine = AudioEngine::new().unwrap();
        
        // Connect to PulseAudio/PipeWire
        let connected = audio_engine.connect().is_ok();
        let available_devices = if connected {
            audio_engine.list_sinks().unwrap_or_default()
        } else {
            Vec::new()
        };

        Self {
            audio_engine: Arc::new(Mutex::new(audio_engine)),
            available_devices,
            selected_device_idx: None,
            is_running: false,
            error_message: if !connected {
                Some("Failed to connect to PulseAudio/PipeWire".to_string())
            } else {
                None
            },
            connected,
            dark_theme: true,
            show_app_mixer: false,
            show_about: false,
            about_tab: 0,
            audio_apps: Vec::new(),
            app_volumes: std::collections::HashMap::new(),
            app_mutes: std::collections::HashMap::new(),
            last_app_update: std::time::Instant::now(),
            last_device_update: std::time::Instant::now(),
            icon_texture: None,
        }
    }

    fn draw_vertical_slider(
        &self,
        ui: &mut egui::Ui,
        channel_num: usize,
        name: &str,
        volume: &mut f32,
        muted: &mut bool,
        audio_level: f32,
    ) {
        let channel_width = 110.0;
        let slider_height = 200.0;
        
        ui.vertical(|ui| {
            ui.set_width(channel_width);
            
            ui.label(egui::RichText::new(format!("Channel {:02}", channel_num + 1))
                .size(14.0)
                .color(if self.dark_theme { 
                    egui::Color32::LIGHT_GRAY 
                } else { 
                    egui::Color32::DARK_GRAY 
                }));
            
            ui.add_space(5.0);
            
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_height(slider_height);
                    
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(25.0);
                            let text_color = if self.dark_theme { 
                                egui::Color32::LIGHT_GRAY 
                            } else { 
                                egui::Color32::DARK_GRAY 
                            };
                            ui.label(egui::RichText::new("10").size(9.0).color(text_color));
                            ui.add_space(slider_height * 0.15);
                            ui.label(egui::RichText::new("5").size(9.0).color(text_color));
                            ui.add_space(slider_height * 0.15);
                            ui.label(egui::RichText::new("0").size(9.0).color(text_color));
                            ui.add_space(slider_height * 0.15);
                            ui.label(egui::RichText::new("-5").size(9.0).color(text_color));
                            ui.add_space(slider_height * 0.15);
                            ui.label(egui::RichText::new("-10").size(9.0).color(text_color));
                        });
                        
                        let slider = egui::Slider::new(volume, 0.0..=2.0)
                            .orientation(egui::SliderOrientation::Vertical)
                            .show_value(false);
                        
                        ui.add_sized([30.0, slider_height], slider);
                    });
                    
                    ui.add_space(5.0);
                    ui.label(egui::RichText::new(format!("{:.0}%", *volume * 100.0))
                        .size(12.0)
                        .color(if *muted { 
                            egui::Color32::DARK_GRAY 
                        } else if self.dark_theme { 
                            egui::Color32::WHITE 
                        } else { 
                            egui::Color32::BLACK 
                        }));
                });
                
                ui.add_space(5.0);
                
                ui.vertical(|ui| {
                    ui.set_height(slider_height);
                    ui.set_width(35.0);
                    
                    let (rect, _response) = ui.allocate_exact_size(
                        egui::vec2(35.0, slider_height),
                        egui::Sense::hover(),
                    );
                    
                    let bg_color = if self.dark_theme { 
                        egui::Color32::from_rgb(20, 25, 30) 
                    } else { 
                        egui::Color32::from_rgb(240, 240, 240) 
                    };
                    ui.painter().rect_filled(rect, 2.0, bg_color);
                    
                    if !*muted && audio_level > 0.01 {
                        let level_height = (audio_level / 2.0).min(1.0) * slider_height;
                        let level_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.min.x, rect.max.y - level_height),
                            egui::vec2(rect.width(), level_height),
                        );
                        
                        let color = if audio_level < 1.2 {
                            egui::Color32::from_rgb(0, 200, 100)
                        } else if audio_level < 1.4 {
                            egui::Color32::from_rgb(200, 200, 0)
                        } else {
                            egui::Color32::from_rgb(200, 0, 0)
                        };
                        
                        ui.painter().rect_filled(level_rect, 2.0, color);
                        
                        for i in 0..20 {
                            let segment_y = rect.max.y - (i as f32 * slider_height / 20.0);
                            if segment_y < rect.max.y - level_height {
                                break;
                            }
                            let line_color = if self.dark_theme { 
                                egui::Color32::from_rgb(10, 15, 20) 
                            } else { 
                                egui::Color32::from_rgb(220, 220, 220) 
                            };
                            ui.painter().hline(
                                rect.min.x..=rect.max.x,
                                segment_y,
                                egui::Stroke::new(1.0, line_color),
                            );
                        }
                    }
                    
                    let border_color = if self.dark_theme { 
                        egui::Color32::from_rgb(60, 65, 70) 
                    } else { 
                        egui::Color32::from_rgb(180, 180, 180) 
                    };
                    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, border_color));
                });
            });
            
            ui.add_space(10.0);
            
            let mute_icon = if *muted { "🔇" } else { "🎧" };
            let button_color = if self.dark_theme {
                egui::Color32::from_rgb(60, 60, 60)
            } else {
                egui::Color32::from_rgb(180, 180, 180)
            };
            let mute_button = egui::Button::new(egui::RichText::new(mute_icon).size(18.0))
                .fill(button_color);
            
            if ui.add_sized([channel_width - 10.0, 35.0], mute_button).clicked() {
                *muted = !*muted;
            }
            
            ui.add_space(5.0);
            
            ui.label(egui::RichText::new(format!("{:02}", channel_num + 1))
                .size(16.0)
                .strong()
                .color(if self.dark_theme { 
                    egui::Color32::LIGHT_GRAY 
                } else { 
                    egui::Color32::DARK_GRAY 
                }));
            
            ui.label(
                egui::RichText::new(name)
                    .size(10.0)
                    .color(if self.dark_theme { 
                        egui::Color32::DARK_GRAY 
                    } else { 
                        egui::Color32::GRAY 
                    })
            ).on_hover_text(name);
        });
    }

    fn draw_app_mixer_window(&mut self, ctx: &egui::Context) {
        let mut show = self.show_app_mixer;
        
        egui::Window::new("Application Mixer")
            .open(&mut show)
            .default_size([450.0, 400.0])
            .show(ctx, |ui| {
                ui.heading("Application Volume Control");
                ui.separator();
                
                if ui.button("🔄 Refresh Now").clicked() {
                    self.update_app_list();
                    self.last_app_update = std::time::Instant::now();
                }
                
                ui.add_space(10.0);
                
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.audio_apps.is_empty() {
                        ui.label("No active applications playing sound");
                        ui.label("Start music or video in any application");
                    } else {
                        // Use a HashSet to track which app indices we've already rendered
                        let mut rendered_indices = std::collections::HashSet::new();
                        
                        for app in self.audio_apps.clone() {
                            // Skip if we've already rendered this app index
                            if !rendered_indices.insert(app.index) {
                                continue;
                            }
                            
                            ui.push_id(app.index, |ui| {
                                ui.group(|ui| {
                                    ui.set_min_width(ui.available_width());
                                    
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(&app.name).size(14.0).strong());
                                        
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let mute_state = self.app_mutes.get(&app.index).copied().unwrap_or(app.muted);
                                            let mute_icon = if mute_state { "🔇" } else { "🔊" };
                                            let button_color = if self.dark_theme {
                                                egui::Color32::from_rgb(60, 60, 60)
                                            } else {
                                                egui::Color32::from_rgb(180, 180, 180)
                                            };
                                            let mute_btn = egui::Button::new(mute_icon)
                                                .fill(button_color);
                                            
                                            if ui.add(mute_btn).clicked() {
                                                let new_mute = !mute_state;
                                                self.app_mutes.insert(app.index, new_mute);
                                                let _ = self.audio_engine.lock().unwrap().set_app_mute(app.index, new_mute);
                                            }
                                        });
                                    });
                                    
                                    let volume = self.app_volumes.entry(app.index).or_insert(app.volume);
                                    let old_volume = *volume;
                                    
                                    ui.add(egui::Slider::new(volume, 0.0..=1.5)
                                        .text("Volume")
                                        .show_value(true)
                                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
                                    
                                    if (*volume - old_volume).abs() > 0.001 {
                                        let _ = self.audio_engine.lock().unwrap().set_app_volume(app.index, *volume);
                                    }
                                });
                            });
                            
                            ui.add_space(5.0);
                        }
                    }
                });
            });
        
        self.show_app_mixer = show;
    }

    fn draw_settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let theme_icon = if self.dark_theme { "🌙" } else { "☀" };
            let theme_text = if self.dark_theme { "Dark" } else { "Light" };
            if ui.button(format!("{} {}", theme_icon, theme_text)).clicked() {
                self.dark_theme = !self.dark_theme;
            }
            
            ui.separator();
            
            if ui.button("App Mixer").clicked() {
                self.show_app_mixer = true;
                self.update_app_list();
            }
            
            ui.separator();
            
            if ui.button("ℹ About").clicked() {
                self.show_about = true;
            }
        });
    }

    fn draw_about_window(&mut self, ctx: &egui::Context) {
        let mut show = self.show_about;
        
        egui::Window::new("About exIOMetter")
            .open(&mut show)
            .default_size([500.0, 400.0])
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let tabs = ["About", "License", "Credits", "Logs"];
                    for (idx, tab_name) in tabs.iter().enumerate() {
                        let selected = self.about_tab == idx;
                        if ui.selectable_label(selected, *tab_name).clicked() {
                            self.about_tab = idx;
                        }
                    }
                });
                
                ui.separator();
                
                match self.about_tab {
                    0 => self.draw_about_tab(ui),
                    1 => self.draw_license_tab(ui),
                    2 => self.draw_credits_tab(ui),
                    3 => self.draw_logs_tab(ui),
                    _ => {}
                }
            });
        
        self.show_about = show;
    }

    fn draw_about_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            
            if self.icon_texture.is_none() {
                let icon_bytes = include_bytes!("../logo/icon.png");
                if let Ok(image) = image::load_from_memory(icon_bytes) {
                    let rgba_image = image.to_rgba8();
                    let texture = egui::ColorImage::from_rgba_unmultiplied(
                        [rgba_image.width() as usize, rgba_image.height() as usize],
                        rgba_image.as_raw(),
                    );
                    
                    self.icon_texture = Some(ui.ctx().load_texture(
                        "app_icon_about",
                        texture,
                        Default::default(),
                    ));
                }
            }
            
            if let Some(texture_handle) = &self.icon_texture {
                ui.add(
                    egui::Image::from_texture(texture_handle)
                        .fit_to_exact_size(egui::Vec2::new(80.0, 80.0))
                );
            }
            
            ui.label(egui::RichText::new("exIOMetter").size(24.0).strong());
            ui.label(egui::RichText::new("v0.0.6").size(12.0).color(egui::Color32::GRAY));
            ui.add_space(10.0);
            
            ui.label("Lite Audio Output Mixer for Linux");
            ui.add_space(15.0);
            
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Features:").size(12.0).strong());
                    ui.label("• Multi-device audio mixing");
                    ui.label("• Per-application volume control");
                    ui.label("• PulseAudio & PipeWire support");
                    ui.label("• Dark & Light themes");
                });
            });
            
            ui.add_space(10.0);
            
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("System:").size(12.0).strong());
                    ui.label(format!("Status: {}", 
                        if self.connected { "Connected" } else { "Disconnected" }
                    ));
                    ui.label(format!("Devices: {}", self.available_devices.len()));
                });
            });
        });
    }

    fn draw_license_tab(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(egui::RichText::new("Nicet Studio PUBLIC LICENSE").size(14.0).strong());
            ui.label(egui::RichText::new("Version 2, June 2026").size(12.0));
            ui.add_space(10.0);
            
            ui.label(
"Copyright (C) 2026 KAInaps

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the \"Software\"), to use, study, copy, modify, merge, and distribute the Software, subject to the following conditions:

ATTRIBUTION REQUIREMENT
The original copyright notice, license text, and attribution to the original author (\"KAInaps\") must be included in all copies, modified versions, forks, and substantial portions of the Software.

Any modified version must clearly indicate that changes were made and identify the authors of those changes. However, no modified version may remove, replace, obscure, or misrepresent the original authorship of the Software.

OPEN SOURCE DISTRIBUTION REQUIREMENT
Any distribution of the Software, whether modified or unmodified, must include the complete corresponding source code.

The source code must be provided under this same license and must be available to all recipients of the distributed Software without additional restrictions.

Distribution of compiled, obfuscated, encrypted, or binary-only versions of the Software without providing the complete corresponding source code is strictly prohibited.

DERIVATIVE WORKS
Users are permitted to create forks, modifications, extensions, and derivative works of the Software.

Derivative works may be distributed, provided that:
- The complete source code is made available
- This license remains attached
- The original author attribution is preserved
- Any modifications are clearly documented

CONTRIBUTIONS
Contributors are encouraged, but not required, to submit improvements, bug fixes, and enhancements to the original project maintained by the original author.

ADDITIONAL RESTRICTIONS
No person or organization may claim exclusive ownership of the Software or any derivative work based substantially upon it.

No person or organization may distribute a derivative work under a more restrictive license that would prevent recipients from accessing, modifying, or redistributing the source code.

DISCLAIMER
THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.

IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE."
            );
        });
    }

    fn draw_credits_tab(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(egui::RichText::new("The exIOMetter").size(16.0).strong());
            ui.add_space(5.0);
            ui.label(egui::RichText::new("by KAInaps").size(12.0));
            ui.add_space(15.0);
            
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Built with:").size(12.0).strong());
                    ui.label("Rust Programming Language");
                    ui.label("egui - Immediate mode UI");
                    ui.label("PulseAudio/PipeWire APIs");
                });
            });
            
            ui.add_space(15.0);
            
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Special Thanks:").size(12.0).strong());
                    ui.label("• egui community");
                    ui.label("• Rust Audio WG");
                    ui.label("• PulseAudio & PipeWire teams");
                    ui.label("• All contributors");
                });
            });
        });
    }

    fn draw_logs_tab(&self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Application Logs").size(12.0).strong());
        ui.add_space(10.0);
        
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label("System Status:");
                        ui.label(format!("  • Audio Server: Connected (PulseAudio/PipeWire)"));
                        ui.label(format!("  • Devices Available: {}", self.available_devices.len()));
                        ui.label(format!("  • Status: {}", 
                            if self.is_running { "Running" } else { "Stopped" }
                        ));
                        
                        if !self.available_devices.is_empty() {
                            ui.add_space(5.0);
                            ui.label("Detected Devices:");
                            for (idx, device) in self.available_devices.iter().enumerate() {
                                ui.label(format!("  {}. {} ({})", idx + 1, device.description, device.name));
                            }
                        }
                        
                        if let Some(error) = &self.error_message {
                            ui.add_space(5.0);
                            ui.colored_label(egui::Color32::RED, "⚠️ Errors:");
                            ui.colored_label(egui::Color32::RED, format!("  {}", error));
                        } else {
                            ui.add_space(5.0);
                            ui.colored_label(egui::Color32::GREEN, "No errors");
                        }
                    });
                });
            });
    }

    fn update_app_list(&mut self) {
        if let Ok(apps) = self.audio_engine.lock().unwrap().list_audio_apps() {
            // Create a set of current app indices
            let current_indices: std::collections::HashSet<u32> = apps.iter().map(|a| a.index).collect();
            
            // Remove stale entries from volume and mute maps
            self.app_volumes.retain(|idx, _| current_indices.contains(idx));
            self.app_mutes.retain(|idx, _| current_indices.contains(idx));
            
            self.audio_apps = apps.clone();
            for app in &apps {
                self.app_volumes.entry(app.index).or_insert(app.volume);
                self.app_mutes.entry(app.index).or_insert(app.muted);
            }
        }
    }
}

impl eframe::App for ExIOMeterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = if self.dark_theme {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        
        visuals.selection.bg_fill = visuals.panel_fill;
        visuals.selection.stroke.width = 0.0;
        ctx.set_visuals(visuals);
        
        if self.show_app_mixer && self.last_app_update.elapsed().as_secs() >= 2 {
            self.update_app_list();
            self.last_app_update = std::time::Instant::now();
        }
        
        if self.last_device_update.elapsed().as_secs() >= 10 {
            if self.connected {
                match self.audio_engine.lock().unwrap().list_sinks() {
                    Ok(devices) => {
                        self.available_devices = devices;
                    }
                    Err(e) => {
                        log::warn!("Error updating device list: {}", e);
                    }
                }
            }
            self.last_device_update = std::time::Instant::now();
        }
        
        let _ = self.audio_engine.lock().unwrap().update_audio_levels();
        
        if self.show_app_mixer {
            self.draw_app_mixer_window(ctx);
        }
        
        if self.show_about {
            self.draw_about_window(ctx);
        }
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.heading("exIOMetter");
                ui.separator();
                ui.label("Audio Output Mixer");
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.draw_settings_panel(ui);
                });
            });
            ui.add_space(5.0);
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                if !self.connected {
                    ui.colored_label(egui::Color32::RED, "❌ Not connected to PulseAudio/PipeWire");
                    return;
                }

                if ui.button(if self.is_running { "⏸ Stop" } else { "▶ Start" }).clicked() {
                    if self.is_running {
                        if let Err(e) = self.audio_engine.lock().unwrap().stop() {
                            self.error_message = Some(format!("Stop error: {}", e));
                        } else {
                            self.is_running = false;
                            self.error_message = None;
                        }
                    } else {
                        match self.audio_engine.lock().unwrap().start() {
                            Ok(_) => {
                                self.is_running = true;
                                self.error_message = None;
                            }
                            Err(e) => {
                                self.error_message = Some(format!("Start error: {}", e));
                            }
                        }
                    }
                }

                if ui.button("🔄 Refresh").clicked() {
                    match self.audio_engine.lock().unwrap().list_sinks() {
                        Ok(devices) => {
                            self.available_devices = devices;
                            self.error_message = None;
                        }
                        Err(e) => {
                            self.error_message = Some(format!("Error: {}", e));
                        }
                    }
                }

                ui.separator();

                // Adding device
                let selected_text = self.selected_device_idx
                    .and_then(|idx| self.available_devices.get(idx))
                    .map(|d| d.description.clone())
                    .unwrap_or_else(|| "Select device...".to_string());

                egui::ComboBox::from_id_source("device_selector")
                    .selected_text(selected_text)
                    .width(250.0)
                    .show_ui(ui, |ui| {
                        for (idx, device) in self.available_devices.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_device_idx, Some(idx), &device.description);
                        }
                    });

                if ui.button("➕ Add").clicked() {
                    if let Some(idx) = self.selected_device_idx {
                        if let Some(device) = self.available_devices.get(idx) {
                            if let Err(e) = self.audio_engine.lock().unwrap()
                                .add_sink(&device.name, &device.description) {
                                self.error_message = Some(format!("Error: {}", e));
                            } else {
                                self.error_message = None;
                            }
                        }
                    }
                }

                // Status
                ui.separator();
                if self.is_running {
                    ui.colored_label(egui::Color32::GREEN, "Active");
                } else {
                    ui.colored_label(egui::Color32::GRAY, "○ Inactive");
                }
            });

            // Ошибки
            if let Some(error) = &self.error_message {
                ui.colored_label(egui::Color32::RED, error);
            }
            
            ui.add_space(5.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.connected {
                return;
            }

            let mut to_remove = None;
            let mut volume_updates = Vec::new();
            let mut mute_updates = Vec::new();
            
            {
                let mut engine = self.audio_engine.lock().unwrap();
                let channels = engine.get_selected_sinks_mut();

                if channels.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(50.0);
                            ui.heading("No devices");
                            ui.label("Add devices below to start working");
                            ui.label("💡 Add 2 or more devices and press 'Start'");
                        });
                    });
                } else {
                    // Адаптивная сетка с переносом вниз
                    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                        let max_width = ui.ctx().screen_rect().width();
                        let panel_margin = 30.0; // Отступы окна
                        let scrollbar_width = 15.0; // Примерная ширина скролбара
                        let channel_width = 145.0; // Ширина одного канала с отступами
                        
                        // Вычисляем количество каналов в ряду с запасом
                        let available_for_channels = max_width - panel_margin - scrollbar_width;
                        let channels_per_row = ((available_for_channels / channel_width).floor() as usize).max(1);
                        
                        // Разбиваем каналы на ряды
                        let mut row_idx = 0;
                        while row_idx * channels_per_row < channels.len() {
                            ui.horizontal(|ui| {
                                ui.set_min_width(available_for_channels);
                                let start_idx = row_idx * channels_per_row;
                                let end_idx = (start_idx + channels_per_row).min(channels.len());
                                
                                for idx in start_idx..end_idx {
                                    let channel = &mut channels[idx];
                                    
                                    ui.group(|ui| {
                                        let mut vol = *channel.volume.lock().unwrap();
                                        let mut muted = *channel.muted.lock().unwrap();
                                        let level = *channel.audio_level.lock().unwrap();
                                        let old_vol = vol;
                                        let old_muted = muted;
                                        
                                        self.draw_vertical_slider(
                                            ui,
                                            idx,
                                            &channel.info.description,
                                            &mut vol,
                                            &mut muted,
                                            level,
                                        );
                                        
                                        // Кнопка удаления
                                        if ui.button("🗑").clicked() {
                                            to_remove = Some(idx);
                                        }
                                        
                                        // Сохраняем изменения только если миксер запущен
                                        if self.is_running {
                                            if (vol - old_vol).abs() > 0.001 {
                                                *channel.volume.lock().unwrap() = vol;
                                                volume_updates.push((channel.sink_index, vol));
                                            }
                                            if muted != old_muted {
                                                *channel.muted.lock().unwrap() = muted;
                                                mute_updates.push((channel.sink_index, muted));
                                            }
                                        } else {
                                            // Если миксер не запущен, восстанавливаем старые значения
                                            *channel.volume.lock().unwrap() = old_vol;
                                            *channel.muted.lock().unwrap() = old_muted;
                                        }
                                    });
                                    
                                    ui.add_space(5.0);
                                }
                            });
                            
                            ui.add_space(10.0);
                            row_idx += 1;
                        }
                    });
                }
            }

            // Применяем изменения громкости и мута
            if !volume_updates.is_empty() || !mute_updates.is_empty() {
                let engine = self.audio_engine.lock().unwrap();
                for (sink_idx, vol) in volume_updates {
                    let _ = engine.set_sink_volume(sink_idx, vol);
                }
                for (sink_idx, muted) in mute_updates {
                    let _ = engine.set_sink_mute(sink_idx, muted);
                }
            }

            // Удаление канала
            if let Some(idx) = to_remove {
                self.audio_engine.lock().unwrap().remove_sink(idx);
                if self.is_running {
                    let _ = self.audio_engine.lock().unwrap().stop();
                    if let Err(e) = self.audio_engine.lock().unwrap().start() {
                        self.error_message = Some(format!("Ошибка: {}", e));
                        self.is_running = false;
                    }
                }
            }
        });

        // Обновление UI с высокой частотой (60 FPS) для плавной анимации VU-метра
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}
