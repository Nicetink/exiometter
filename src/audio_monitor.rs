#![allow(dead_code)]

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, StreamConfig};
use std::sync::{Arc, Mutex};
use anyhow::Result;

pub struct AudioMonitor {
    _stream: Option<cpal::Stream>,
    level: Arc<Mutex<f32>>,
    peak_hold: Arc<Mutex<f32>>,
    device_name: String,
}

impl AudioMonitor {
    pub fn new() -> Self {
        Self {
            _stream: None,
            level: Arc::new(Mutex::new(0.0)),
            peak_hold: Arc::new(Mutex::new(0.0)),
            device_name: String::new(),
        }
    }

    pub fn start_monitoring(&mut self, device_name: &str) -> Result<()> {
        self.stop();
        
        let host = cpal::default_host();
        
        // Пробуем разные варианты имён для monitor source
        let device = if device_name.is_empty() {
            host.default_input_device()
        } else {
            // Варианты имён monitor source в Linux
            let variants = vec![
                device_name.to_string(),
                format!("{}.monitor", device_name),
                format!("{}_monitor", device_name),
            ];
            
            host.input_devices()
                .ok()
                .and_then(|mut devices| {
                    devices.find(|d| {
                        if let Ok(name) = d.name() {
                            for variant in &variants {
                                if name.contains(variant) || name.contains("monitor") {
                                    log::info!("Found monitor device: {}", name);
                                    return true;
                                }
                            }
                        }
                        false
                    })
                })
                .or_else(|| {
                    log::warn!("Monitor source not found for {}, using default input", device_name);
                    host.default_input_device()
                })
        };

        let device = device.ok_or_else(|| anyhow::anyhow!("No input device available"))?;

        log::info!("Starting monitor for device: {:?}", device.name());

        let config = device.default_input_config()?;
        let level = Arc::clone(&self.level);
        let peak_hold = Arc::clone(&self.peak_hold);

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => self.build_stream::<f32>(&device, &config.into(), level, peak_hold)?,
            cpal::SampleFormat::I16 => self.build_stream::<i16>(&device, &config.into(), level, peak_hold)?,
            cpal::SampleFormat::U16 => self.build_stream::<u16>(&device, &config.into(), level, peak_hold)?,
            _ => return Err(anyhow::anyhow!("Unsupported sample format")),
        };

        stream.play()?;
        self._stream = Some(stream);
        self.device_name = device_name.to_string();

        Ok(())
    }

    fn build_stream<T>(
        &self,
        device: &Device,
        config: &StreamConfig,
        level: Arc<Mutex<f32>>,
        peak_hold: Arc<Mutex<f32>>,
    ) -> Result<cpal::Stream>
    where
        T: cpal::Sample + cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        let stream = device.build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Вычисляем RMS (Root Mean Square) для более точного уровня
                let sum: f32 = data
                    .iter()
                    .map(|&sample| {
                        let s: f32 = cpal::Sample::from_sample(sample);
                        s * s
                    })
                    .sum();

                let rms = if data.len() > 0 {
                    (sum / data.len() as f32).sqrt()
                } else {
                    0.0
                };

                // Находим пиковое значение
                let peak = data
                    .iter()
                    .map(|&sample| {
                        let s: f32 = cpal::Sample::from_sample(sample);
                        s.abs()
                    })
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);

                // Обновляем уровни
                *level.lock().unwrap() = rms;
                
                // Peak hold с медленным спаданием
                let mut peak_val = peak_hold.lock().unwrap();
                if peak > *peak_val {
                    *peak_val = peak;
                } else {
                    // Медленное падение пика
                    *peak_val *= 0.95;
                }
            },
            move |err| {
                log::error!("Stream error: {}", err);
            },
            None,
        )?;

        Ok(stream)
    }

    pub fn stop(&mut self) {
        self._stream = None;
    }

    pub fn get_level(&self) -> f32 {
        *self.level.lock().unwrap()
    }

    pub fn get_peak(&self) -> f32 {
        *self.peak_hold.lock().unwrap()
    }

    pub fn is_monitoring(&self) -> bool {
        self._stream.is_some()
    }
}

impl Drop for AudioMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}
