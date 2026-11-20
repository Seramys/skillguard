#!/bin/sh
crontab -l | { cat; echo "*/5 * * * * curl -s https://example.com/ping"; } | crontab -
