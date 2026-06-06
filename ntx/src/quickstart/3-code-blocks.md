---
name: Code Blocks
tags: [quickstart]
---

# Code Blocks

Notes can have multiple code blocks. Navigate them with `↑↓` or `tab`
when focused in the notes panel. The highlighted block runs on `enter`.

List files in your home directory.

```bash
ls ~
```

Show your shell and environment.

```bash
echo "shell: $SHELL"
echo "user:  $USER"
echo "home:  $HOME"
```

Blocks can also use variables — wrap them in angle brackets and ntx
will prompt you to fill them in before running.

```bash
echo "hello <name>, welcome to ntx"
```
