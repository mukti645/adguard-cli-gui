# AdGuard CLI GUI

Графическая оболочка для управления [adguard-cli](https://adguard.com/kb/adguard-for-linux/) на Linux.
Написана на **Rust + iced** (Elm-архитектура). Тёмная тема Catppuccin Mocha.

![Rust](https://img.shields.io/badge/Rust-2024-orange)
![iced](https://img.shields.io/badge/iced-0.13-blue)
![License](https://img.shields.io/badge/License-MIT-yellow)

---

## Возможности

| Вкладка | Функционал |
|---------|-----------|
| **Status** | Включить / выключить защиту, просмотр статуса, автоустановка adguard-cli |
| **License** | Активация лицензии, сброс, просмотр текущей лицензии |
| **Filters** | Список установленных фильтров |
| **Updates** | Проверка обновлений, установка, экспорт логов |

---

## Требования

- Linux (Arch / CachyOS / любой дистрибутив)
- Rust 1.70+ (для сборки)
- [adguard-cli](https://adguard.com/kb/adguard-for-linux/) — устанавливается автоматически при первом запуске
- Графическая среда (X11 или Wayland)

---

## Установка

### 1. Клонировать репозиторий

```bash
git clone https://github.com/mukti645/adguard-cli-gui.git
cd adguard-cli-gui
```

### 2. Собрать и установить

```bash
cargo build --release

# Установить бинарь
cp target/release/adguard-cli-gui ~/.local/bin/

# Или глобально
sudo cp target/release/adguard-cli-gui /usr/local/bin/
```

### 3. Запустить

```bash
adguard-cli-gui
# или
./run.sh  # установит adguard-cli если отсутствует
```

---

## Установка adguard-cli

GUI автоматически предложит установить `adguard-cli` если он не найден.

**Arch / CachyOS (вручную):**
```bash
paru -S adguard-cli-bin
# или
yay -S adguard-cli-bin
```

**Официальный скрипт:**
```bash
curl -fsSL https://raw.githubusercontent.com/AdguardTeam/AdGuardCLI/release/install.sh | sh -s -- -v
```

### Настроить sudo без пароля (рекомендуется)

adguard-cli требует root для некоторых операций:

```bash
echo "$USER ALL=(ALL) NOPASSWD: /opt/adguard-cli/adguard-cli" | sudo tee /etc/sudoers.d/99-adguard
sudo chmod 440 /etc/sudoers.d/99-adguard
```

---

## Структура

```
adguard-cli-gui/
├── Cargo.toml
├── src/
│   ├── main.rs     # Точка входа
│   ├── app.rs      # iced приложение (Elm-архитектура)
│   ├── cli.rs      # Обёртка над adguard-cli
│   └── theme.rs    # Цвета Catppuccin Mocha
├── run.sh          # Скрипт запуска с проверкой зависимостей
└── README.md
```

---

## Лицензия

MIT
