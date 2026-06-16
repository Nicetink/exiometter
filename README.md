/home/user/combine-audio.sh (объединить)

bash
#!/bin/bash
pactl load-module module-combine-sink sink_name=combined sinks=alsa_output.pci-0000_00_1f.3.analog-stereo,alsa_output.pci-0000_01_00.1.hdmi-stereo
pactl set-default-sink combined
/home/user/separate-audio.sh (разъединить)

bash
#!/bin/bash
ID=$(pactl list short modules | grep combine | awk '{print $1}')
pactl unload-module $ID
pactl set-default-sink alsa_output.pci-0000_00_1f.3.analog-stereo# exIOMetter - Audio Output Mixer

Программа для Linux для объединения нескольких аудиовыходов с индивидуальными микшерами для каждого канала.

## Возможности

- ✅ Объединение 2 и более аудиовыходов
- ✅ Одновременное воспроизведение звука на всех подключенных устройствах
- ✅ Индивидуальный микшер для каждого канала:
  - Регулировка громкости (0-200%)
  - Кнопка Mute для каждого канала
  - Визуальный индикатор уровня
- ✅ Графический интерфейс на основе egui
- ✅ Добавление/удаление устройств в реальном времени
- ✅ Обновление списка доступных устройств

## Требования

- Rust (версия 1.70 или новее)
- ALSA или PulseAudio/PipeWire для аудио на Linux
- Зависимости для сборки:
  ```bash
  sudo apt install libasound2-dev pkg-config  # Debian/Ubuntu
  sudo dnf install alsa-lib-devel             # Fedora
  sudo pacman -S alsa-lib                     # Arch Linux
  ```

## Установка

1. Клонируйте репозиторий:
```bash
git clone <repository-url>
cd exiometter
```

2. Соберите проект:
```bash
cargo build --release
```

3. Запустите программу:
```bash
cargo run --release
```

## Использование

1. **Запустите приложение** - откроется графический интерфейс

2. **Добавьте устройства вывода**:
   - Выберите устройство из выпадающего списка "Available Devices"
   - Нажмите кнопку "➕ Add Device"
   - Повторите для всех нужных устройств

3. **Настройте микшеры**:
   - Для каждого добавленного устройства доступны:
     - Слайдер громкости (0-200%)
     - Чекбокс Mute для отключения звука
     - Визуальный индикатор уровня
   - Кнопка "🗑 Remove" для удаления устройства

4. **Запустите микшер**:
   - Нажмите кнопку "▶ Start"
   - Звук с входного устройства будет направлен на все добавленные выходы
   - Нажмите "⏸ Stop" для остановки

5. **Обновите список устройств**:
   - Нажмите "🔄 Refresh Devices" если подключили новое устройство

## Архитектура

- **src/main.rs** - точка входа приложения
- **src/audio_engine.rs** - движок обработки аудио с использованием cpal
- **src/ui.rs** - графический интерфейс на egui

## Технологии

- **Rust** - язык программирования
- **egui/eframe** - графический интерфейс
- **cpal** - кроссплатформенная библиотека для работы с аудио
- **ringbuf** - ring buffer для передачи аудиоданных между потоками
- **rubato** - ресемплинг (зарезервировано для будущих версий)

## Лицензия

Nicet Studio PUBLIC LICENSE Version 2, June 2026
Copyright (C) 2026 KAInaps

## Автор

KAInaps
