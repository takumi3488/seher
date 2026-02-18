#!/bin/bash

# Run only in remote environment
if [ "$CLAUDE_CODE_REMOTE" != "true" ]; then
  exit 0
fi

# Install taplo
cargo install taplo-cli
exit 0
