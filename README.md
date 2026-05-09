# MiniPWN

MiniPWN is an autonomous pentesting agent designed for security professionals. It provides a terminal-based interface (TUI) to interact with AI models that can execute tools locally or via remote workers to assist in security assessments.

## Features

- Multi-provider support (OpenAI, OpenRouter, Custom endpoints).
- Modular tool execution framework.
- Remote worker support for distributed tasks.
- Advanced TUI with markdown rendering and theme support.
- Configurable agentic loops with safety/weaponized modes.

## Installation

### From Cargo

The easiest way to install MiniPWN is via cargo:

```bash
cargo install minipwn
```

### From Source

To build and install MiniPWN from the source code, follow these steps:

1. Clone the repository:
```bash
git clone https://github.com/sammwyy/minipwn.git
cd minipwn
```

2. Build and install:
```bash
cargo install --path .
```

## Configuration

While you can edit the configuration files manually in your user config folder (e.g., `~/.config/minipwn` on Linux), it is recommended to configure MiniPWN directly from the TUI:

1. Run `minipwn`.
2. Use `/provider` to select your preferred AI provider.
3. Use `/apikey <your-key>` to set the secret key for the currently selected provider.
4. Use `/model` to select the specific model you wish to use.

Alternatively, you can manually configure `secrets.env`:

```env
OPENAI_SECRETKEY="your-key-here"
OPENROUTER_SECRETKEY="your-key-here"
```

And customize behavior in `config.toml`:

```toml
provider = "openai"
theme = "dracula"
max_iterations = 25

[tui]
max_history_display = 10
```

## Usage

Simply run the binary to start the interactive TUI:

```bash
minipwn
```

Inside the TUI, you can use commands by pressing `/`:

- `/help`: Show available commands.
- `/mode`: Toggle between Safe and Weaponized modes.
- `/provider`: Change the current AI provider.
- `/apikey`: Set the API key for the current provider.
- `/model`: Change the current AI model.
- `/theme`: Change the UI theme.

## License

This project is licensed under the MIT License.
