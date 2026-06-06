---
name: Your Projects
tags: [quickstart]
---

# Your Projects

Projects live in ~/.ntx/projects/. Each one has a config file that
sets the working directory, shell, env vars, and a startup script.

Create a project from the browser with `n`, then add a config file at:

  ~/.ntx/projects/<project>/config/default.conf

Example config:

  dir = ~/dev/my-project
  shell = /bin/zsh

  [env]
  NODE_ENV = development

  [script]
  echo "ready"

Notes go in:

  ~/.ntx/projects/<project>/notes/

Check where your projects are stored.

```bash
ls ~/.ntx/projects/
```

See what's in this quickstart project.

```bash
ls ~/.ntx/projects/quickstart/
```
