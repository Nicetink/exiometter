use libpulse_binding as pulse;
use pulse::context::{Context, FlagSet as ContextFlagSet};
use pulse::mainloop::threaded::Mainloop;
use pulse::proplist::Proplist;
use pulse::volume::{ChannelVolumes, Volume};
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::cell::RefCell;
use anyhow::{Result, Context as AnyhowContext};

#[derive(Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub description: String,
}

#[derive(Clone)]
pub struct AppAudioInfo {
    pub name: String,
    pub index: u32,
    pub volume: f32,
    pub muted: bool,
}

pub struct SinkChannel {
    pub info: DeviceInfo,
    pub volume: Arc<Mutex<f32>>,
    pub muted: Arc<Mutex<bool>>,
    pub audio_level: Arc<Mutex<f32>>,
    pub sink_index: u32,
}

pub struct AudioEngine {
    mainloop: Option<Rc<RefCell<Mainloop>>>,
    context: Option<Rc<RefCell<Context>>>,
    combined_sink_name: String,
    combined_module_index: Option<u32>,
    available_sinks: Vec<DeviceInfo>,
    selected_sinks: Vec<SinkChannel>,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            mainloop: None,
            context: None,
            combined_sink_name: "exiometter_combined".to_string(),
            combined_module_index: None,
            available_sinks: Vec::new(),
            selected_sinks: Vec::new(),
        })
    }

    pub fn connect(&mut self) -> Result<()> {
        let mut mainloop = Mainloop::new().context("Failed to create PulseAudio mainloop")?;
        
        let mut proplist = Proplist::new().unwrap();
        proplist.set_str(pulse::proplist::properties::APPLICATION_NAME, "exIOMetter")
            .unwrap();
        
        let mut context = Context::new_with_proplist(&mainloop, "exIOMetter", &proplist)
            .context("Failed to create PulseAudio context")?;
        
        context.connect(None, ContextFlagSet::NOFLAGS, None)
            .context("Failed to connect to PulseAudio/PipeWire")?;
        
        mainloop.lock();
        mainloop.start()
            .context("Failed to start mainloop")?;
        
        // Wait for connection
        loop {
            match context.get_state() {
                pulse::context::State::Ready => break,
                pulse::context::State::Failed | pulse::context::State::Terminated => {
                    mainloop.unlock();
                    return Err(anyhow::anyhow!("Failed to connect to PulseAudio/PipeWire"));
                }
                _ => {
                    mainloop.unlock();
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    mainloop.lock();
                }
            }
        }
        mainloop.unlock();
        
        self.mainloop = Some(Rc::new(RefCell::new(mainloop)));
        self.context = Some(Rc::new(RefCell::new(context)));
        
        log::info!("Connected to PulseAudio/PipeWire");
        Ok(())
    }

    pub fn list_sinks(&mut self) -> Result<Vec<DeviceInfo>> {
        let context = self.context.as_ref()
            .context("Context not initialized")?;
        let _mainloop = self.mainloop.as_ref()
            .context("Mainloop not initialized")?;
        
        let sinks = Arc::new(Mutex::new(Vec::new()));
        let sinks_clone = Arc::clone(&sinks);
        let done = Arc::new(Mutex::new(false));
        let done_clone = Arc::clone(&done);
        
        let cards = Arc::new(Mutex::new(Vec::new()));
        let cards_clone = Arc::clone(&cards);
        let cards_done = Arc::new(Mutex::new(false));
        let cards_done_clone = Arc::clone(&cards_done);
        
        let introspector = context.borrow().introspect();
        
        introspector.get_card_info_list(move |list| {
            match list {
                pulse::callbacks::ListResult::Item(item) => {
                    if let Some(card_name) = &item.name {
                        log::info!("Found sound card: {}", card_name);
                        
                        for profile in &item.profiles {
                            if let Some(name) = &profile.name {
                                log::info!("  Profile: {}", name);
                                if let Some(desc) = &profile.description {
                                    log::info!("    Description: {}", desc);
                                }
                            }
                        }
                        
                        for port in &item.ports {
                            if let Some(port_name) = &port.name {
                                let port_desc = port.description.as_deref().unwrap_or("");
                                log::info!("  Port: {} ({})", port_name, port_desc);
                                if port_name.contains("analog") {
                                    cards_clone.lock().unwrap().push(format!("{}:{}", card_name, port_name));
                                }
                            }
                        }
                    }
                }
                pulse::callbacks::ListResult::End => {
                    *cards_done_clone.lock().unwrap() = true;
                }
                pulse::callbacks::ListResult::Error => {
                    *cards_done_clone.lock().unwrap() = true;
                }
            }
        });
        
        let timeout = std::time::Duration::from_secs(3);
        let start = std::time::Instant::now();
        while !*cards_done.lock().unwrap() && start.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        let introspector = context.borrow().introspect();
        introspector.get_sink_info_list(move |list| {
            match list {
                pulse::callbacks::ListResult::Item(item) => {
                    let name = item.name.as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    
                    if !name.starts_with("exiometter_combined") {
                        let description = item.description.as_ref()
                            .map(|s| Self::format_device_name(&s.to_string()))
                            .unwrap_or_else(|| name.clone());
                        
                        for port in &item.ports {
                            if let Some(port_name) = &port.name {
                                let port_desc = port.description.as_deref().unwrap_or("");
                                
                                if port_name.contains("analog-output") {
                                    let detailed_name = format!("{}#{}", name, port_name);
                                    let detailed_desc = Self::get_port_description(port_name, port_desc);
                                    
                                    sinks_clone.lock().unwrap().push(DeviceInfo {
                                        name: detailed_name,
                                        description: format!("{} - {}", description, detailed_desc),
                                    });
                                }
                            }
                        }
                        
                        if !item.ports.iter().any(|p| {
                            p.name.as_ref().map_or(false, |name| name.contains("analog-output"))
                        }) {
                            sinks_clone.lock().unwrap().push(DeviceInfo {
                                name,
                                description,
                            });
                        }
                    }
                }
                pulse::callbacks::ListResult::End => {
                    *done_clone.lock().unwrap() = true;
                }
                pulse::callbacks::ListResult::Error => {
                    *done_clone.lock().unwrap() = true;
                }
            }
        });
        
        let timeout = std::time::Duration::from_secs(2);
        let start = std::time::Instant::now();
        while !*done.lock().unwrap() && start.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        let result = sinks.lock().unwrap().clone();
        self.available_sinks = result.clone();
        
        log::info!("Found {} audio devices", result.len());
        for sink in &result {
            log::info!("  {}: {}", sink.name, sink.description);
        }
        
        Ok(result)
    }

    fn get_port_description(port_name: &str, port_desc: &str) -> String {
        match port_name {
            name if name.contains("lineout") => "🟢 Line Out (green jack)".to_string(),
            name if name.contains("headphones") => "🎧 Headphones (green jack)".to_string(),
            name if name.contains("speaker") && name.contains("front") => "🟢 Front Speakers (green)".to_string(),
            name if name.contains("speaker") && name.contains("rear") => "🔵 Rear Speakers (blue)".to_string(),
            name if name.contains("speaker") && name.contains("surround") => "⚫ Surround (black)".to_string(),
            name if name.contains("speaker") && name.contains("side") => "⚫ Side Speakers (gray)".to_string(),
            name if name.contains("center-lfe") => "🟠 Center + Sub (orange)".to_string(),
            name if name.contains("mic") => "🔴 Microphone (pink)".to_string(),
            name if name.contains("line-in") => "🔵 Line In (blue)".to_string(),
            _ => {
                if port_desc.is_empty() {
                    format!("🔊 {}", port_name)
                } else {
                    format!("🔊 {}", port_desc)
                }
            }
        }
    }

    fn format_device_name(name: &str) -> String {
        name.replace("Built-in Audio ", "")
            .replace("Analog Stereo", "Analog Stereo")
            .replace("Digital Stereo", "Digital Stereo")
            .replace("HDMI", "HDMI")
            .replace("Headphones", "Headphones")
    }

    pub fn add_sink(&mut self, sink_name: &str, description: &str) -> Result<()> {
        let (actual_sink_name, port_name) = if sink_name.contains('#') {
            let parts: Vec<&str> = sink_name.split('#').collect();
            (parts[0], Some(parts[1]))
        } else {
            (sink_name, None)
        };
        
        let sink_index = self.get_sink_index(actual_sink_name)?;
        
        if let Some(port) = port_name {
            self.set_sink_port(sink_index, port)?;
        }
        
        let channel = SinkChannel {
            info: DeviceInfo {
                name: sink_name.to_string(),
                description: description.to_string(),
            },
            volume: Arc::new(Mutex::new(1.0)),
            muted: Arc::new(Mutex::new(false)),
            audio_level: Arc::new(Mutex::new(0.0)),
            sink_index,
        };
        
        self.selected_sinks.push(channel);
        Ok(())
    }

    fn set_sink_port(&self, sink_index: u32, port_name: &str) -> Result<()> {
        let context = self.context.as_ref()
            .context("Context not initialized")?;
        
        log::info!("Switching sink {} to port {}", sink_index, port_name);
        
        let mut introspector = context.borrow().introspect();
        introspector.set_sink_port_by_index(sink_index, port_name, None);
        
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        Ok(())
    }

    fn get_sink_index(&self, sink_name: &str) -> Result<u32> {
        let context = self.context.as_ref()
            .context("Context not initialized")?;
        
        let actual_sink_name = if sink_name.contains('#') {
            sink_name.split('#').next().unwrap()
        } else {
            sink_name
        };
        
        let index = Arc::new(Mutex::new(None));
        let index_clone = Arc::clone(&index);
        let done = Arc::new(Mutex::new(false));
        let done_clone = Arc::clone(&done);
        let sink_name_owned = actual_sink_name.to_string();
        
        let introspector = context.borrow().introspect();
        introspector.get_sink_info_list(move |list| {
            match list {
                pulse::callbacks::ListResult::Item(item) => {
                    if let Some(name) = &item.name {
                        if name.to_string() == sink_name_owned {
                            *index_clone.lock().unwrap() = Some(item.index);
                        }
                    }
                }
                pulse::callbacks::ListResult::End => {
                    *done_clone.lock().unwrap() = true;
                }
                pulse::callbacks::ListResult::Error => {
                    *done_clone.lock().unwrap() = true;
                }
            }
        });
        
        let timeout = std::time::Duration::from_secs(2);
        let start = std::time::Instant::now();
        while !*done.lock().unwrap() && start.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        let idx = index.lock().unwrap();
        match *idx {
            Some(i) => Ok(i),
            None => Err(anyhow::anyhow!("Sink not found")),
        }
    }

    pub fn remove_sink(&mut self, index: usize) {
        if index < self.selected_sinks.len() {
            self.selected_sinks.remove(index);
        }
    }

    pub fn get_selected_sinks_mut(&mut self) -> &mut Vec<SinkChannel> {
        &mut self.selected_sinks
    }

    pub fn set_sink_volume(&self, sink_index: u32, volume: f32) -> Result<()> {
        let context = self.context.as_ref()
            .context("Context not initialized")?;
        
        // Apply cubic curve for better volume control (human hearing is logarithmic)
        // This gives smooth control from 0-100% and allows boost above 100%
        let adjusted_volume = if volume <= 1.0 {
            // 0-100%: cubic curve for smooth perception
            volume.powf(3.0)
        } else {
            // 100-200%: linear scaling above 100%
            1.0 + (volume - 1.0)
        };
        
        // Convert 0.0-2.0 to PulseAudio Volume (0-65536*2)
        let pa_volume = (adjusted_volume * Volume::NORMAL.0 as f32) as u32;
        let pa_volume = Volume(pa_volume.min(Volume::NORMAL.0 * 2));
        
        let mut channel_volumes = ChannelVolumes::default();
        channel_volumes.set(2, pa_volume);
        
        let mut introspector = context.borrow().introspect();
        introspector.set_sink_volume_by_index(sink_index, &channel_volumes, None);
        
        Ok(())
    }

    pub fn set_sink_mute(&self, sink_index: u32, muted: bool) -> Result<()> {
        let context = self.context.as_ref()
            .context("Context not initialized")?;
        
        let mut introspector = context.borrow().introspect();
        introspector.set_sink_mute_by_index(sink_index, muted, None);
        
        Ok(())
    }

    pub fn update_audio_levels(&mut self) -> Result<()> {
        let mixer_active = self.combined_module_index.is_some();
        
        if mixer_active {
            for channel in self.selected_sinks.iter_mut() {
                let volume = *channel.volume.lock().unwrap();
                let muted = *channel.muted.lock().unwrap();
                
                if muted || volume < 0.01 {
                    let current = *channel.audio_level.lock().unwrap();
                    *channel.audio_level.lock().unwrap() = current * 0.9;
                } else {
                    *channel.audio_level.lock().unwrap() = volume;
                }
            }
        } else {
            for channel in &mut self.selected_sinks {
                let current = *channel.audio_level.lock().unwrap();
                *channel.audio_level.lock().unwrap() = current * 0.85;
            }
        }
        
        Ok(())
    }



    pub fn start(&mut self) -> Result<()> {
        if self.selected_sinks.is_empty() {
            return Err(anyhow::anyhow!("Add at least one device"));
        }

        self.stop()?;

        let sink_names: Vec<String> = self.selected_sinks.iter()
            .map(|s| {
                if s.info.name.contains('#') {
                    s.info.name.split('#').next().unwrap().to_string()
                } else {
                    s.info.name.clone()
                }
            })
            .collect();
        let sinks_arg = sink_names.join(",");
        
        log::info!("Creating combined sink with devices: {}", sinks_arg);
        
        // Load module-combine-sink with parameters to eliminate volume effect
        // adjust_time=0 - disables automatic synchronization (removes delays)
        // resample_method=trivial - minimal resampling (removes distortion)
        let module_args = format!("sink_name={} slaves={} adjust_time=0 resample_method=trivial", 
                                  self.combined_sink_name, sinks_arg);
        
        let done = Arc::new(Mutex::new(false));
        let done_clone = Arc::clone(&done);
        let module_index = Arc::new(Mutex::new(None));
        let module_index_clone = Arc::clone(&module_index);
        
        {
            let context = self.context.as_ref()
                .context("Контекст не инициализирован")?;
            
            let mut introspector = context.borrow().introspect();
            introspector.load_module(
                "module-combine-sink",
                &module_args,
                move |idx| {
                    *module_index_clone.lock().unwrap() = Some(idx);
                    *done_clone.lock().unwrap() = true;
                    log::info!("Модуль загружен с индексом: {}", idx);
                }
            );
        }
        
        let timeout = std::time::Duration::from_secs(3);
        let start = std::time::Instant::now();
        while !*done.lock().unwrap() && start.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        let idx = module_index.lock().unwrap();
        if let Some(module_idx) = *idx {
            self.combined_module_index = Some(module_idx);
            
            std::thread::sleep(std::time::Duration::from_millis(500));
            
            // Apply current volume and mute settings to each sink
            for channel in &self.selected_sinks {
                let volume = *channel.volume.lock().unwrap();
                let muted = *channel.muted.lock().unwrap();
                
                log::info!("Initializing sink {} with volume {} and mute {}", 
                          channel.sink_index, volume, muted);
                
                let _ = self.set_sink_volume(channel.sink_index, volume);
                let _ = self.set_sink_mute(channel.sink_index, muted);
            }
            
            self.start_monitoring()?;
            
            std::thread::sleep(std::time::Duration::from_millis(200));
            
            let combined_name = self.combined_sink_name.clone();
            let done2 = Arc::new(Mutex::new(false));
            let done2_clone = Arc::clone(&done2);
            
            {
                let context = self.context.as_ref().unwrap();
                context.borrow_mut().set_default_sink(&combined_name, move |success| {
                    if success {
                        log::info!("Combined sink set as default device");
                    } else {
                        log::warn!("Failed to set combined sink as default");
                    }
                    *done2_clone.lock().unwrap() = true;
                });
            }
            
            let start2 = std::time::Instant::now();
            while !*done2.lock().unwrap() && start2.elapsed() < std::time::Duration::from_secs(2) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to load combine-sink module"))
        }
    }

    fn start_monitoring(&mut self) -> Result<()> {
        log::info!("Audio level monitoring started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(module_idx) = self.combined_module_index {
            let context = self.context.as_ref()
                .context("Context not initialized")?;
            
            log::info!("Unloading module with index: {}", module_idx);
            
            if let Some(first_sink) = self.selected_sinks.first() {
                let sink_name = first_sink.info.name.clone();
                let done_default = Arc::new(Mutex::new(false));
                let done_default_clone = Arc::clone(&done_default);
                
                context.borrow_mut().set_default_sink(&sink_name, move |_| {
                    log::info!("Default device restored");
                    *done_default_clone.lock().unwrap() = true;
                });
                
                let start = std::time::Instant::now();
                while !*done_default.lock().unwrap() && start.elapsed() < std::time::Duration::from_secs(1) {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            
            let done = Arc::new(Mutex::new(false));
            let done_clone = Arc::clone(&done);
            
            let mut introspector = context.borrow().introspect();
            introspector.unload_module(module_idx, move |success| {
                if success {
                    log::info!("Module unloaded successfully");
                } else {
                    log::warn!("Failed to unload module");
                }
                *done_clone.lock().unwrap() = true;
            });
            
            let timeout = std::time::Duration::from_secs(2);
            let start = std::time::Instant::now();
            while !*done.lock().unwrap() && start.elapsed() < timeout {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            
            self.combined_module_index = None;
        }
        Ok(())
    }

    pub fn list_audio_apps(&self) -> Result<Vec<AppAudioInfo>> {
        let context = self.context.as_ref()
            .context("Context not initialized")?;
        
        let apps = Arc::new(Mutex::new(Vec::new()));
        let apps_clone = Arc::clone(&apps);
        let done = Arc::new(Mutex::new(false));
        let done_clone = Arc::clone(&done);
        
        let introspector = context.borrow().introspect();
        introspector.get_sink_input_info_list(move |list| {
            match list {
                pulse::callbacks::ListResult::Item(item) => {
                    let app_name = item.proplist.get_str("application.name")
                        .or_else(|| item.proplist.get_str("media.name"))
                        .unwrap_or_else(|| format!("Application {}", item.index));
                    
                    if !app_name.contains("exIOMetter") && !app_name.contains("exiometter_combined") {
                        let volume_avg = item.volume.avg().0 as f32 / Volume::NORMAL.0 as f32;
                        
                        apps_clone.lock().unwrap().push(AppAudioInfo {
                            name: app_name,
                            index: item.index,
                            volume: volume_avg,
                            muted: item.mute,
                        });
                    }
                }
                pulse::callbacks::ListResult::End => {
                    *done_clone.lock().unwrap() = true;
                }
                pulse::callbacks::ListResult::Error => {
                    *done_clone.lock().unwrap() = true;
                }
            }
        });
        
        let timeout = std::time::Duration::from_secs(2);
        let start = std::time::Instant::now();
        while !*done.lock().unwrap() && start.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        let result = apps.lock().unwrap().clone();
        Ok(result)
    }

    pub fn set_app_volume(&self, app_index: u32, volume: f32) -> Result<()> {
        let context = self.context.as_ref()
            .context("Context not initialized")?;
        
        let pa_volume = (volume * Volume::NORMAL.0 as f32) as u32;
        let pa_volume = Volume(pa_volume.min(Volume::NORMAL.0 * 2));
        
        let mut channel_volumes = ChannelVolumes::default();
        channel_volumes.set(2, pa_volume);
        
        let mut introspector = context.borrow().introspect();
        introspector.set_sink_input_volume(app_index, &channel_volumes, None);
        
        Ok(())
    }

    pub fn set_app_mute(&self, app_index: u32, muted: bool) -> Result<()> {
        let context = self.context.as_ref()
            .context("Context not initialized")?;
        
        let mut introspector = context.borrow().introspect();
        introspector.set_sink_input_mute(app_index, muted, None);
        
        Ok(())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        // Ignore all errors during cleanup
        if let Err(e) = self.stop() {
            log::warn!("Error stopping in Drop: {}", e);
        }
        
        if let Some(mainloop) = &self.mainloop {
            mainloop.borrow_mut().stop();
        }
    }
}
