# Finder

A CLI tool for searching and chatting with your markdown files.

![Search Mode](assets/search.png)

![Neovim](assets/neovim.png)

![Chat Mode](assets/chat.png)

## What it does

**Search** - Find text across all `.md` files in the current directory. Type and see results instantly. Case-insensitive, fuzzy matching, works with partial words. Press Enter to open the file in Neovim at the exact line.

**Chat** - Ask questions about your documents using AI. All markdown files are loaded as context, and you can chat with Gemini Flash 3 with streaming responses.

## Installation

```bash
git clone https://github.com/CairoAC/finder.git
cd finder
cargo build --release
cp target/release/f ~/.local/bin/
```

## Usage

```bash
cd ~/your-docs
f
```

### Search Mode

| Key | Action |
|-----|--------|
| Type | Search |
| `Up/Down` | Navigate |
| `Enter` | Open in Neovim |
| `?` | Chat mode |
| `Esc` | Quit |

Mouse scroll and click also work.

### Chat Mode

Press `?` to chat with your documents.

| Key | Action |
|-----|--------|
| Type | Write question |
| `Enter` | Send |
| `Ctrl+C` | Cancel / back |
| `Esc` | Back to search |

## Chat Setup

For chat mode, add your OpenRouter API key:

```bash
echo 'OPENROUTER_API_KEY=sk-or-...' >> ~/.env
```

## Dependencies

- Rust 1.70+
- Neovim
- OpenRouter API key (optional, for chat)

## License

MIT
