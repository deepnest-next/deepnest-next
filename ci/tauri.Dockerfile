# SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Deepnest-Commercial
# Build/test image for the desktop app (webkit2gtk + tauri-driver for e2e). Private git
# dependencies are fetched over SSH: run with -v $SSH_AUTH_SOCK:/ssh-agent -e SSH_AUTH_SOCK=/ssh-agent.
FROM rust:1-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
      libayatana-appindicator3-dev librsvg2-dev libssl-dev pkg-config build-essential curl \
      openssh-client git xvfb \
  && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /root/.ssh && ssh-keyscan github.com >> /root/.ssh/known_hosts
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true \
    GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=url.git@github.com:.insteadOf GIT_CONFIG_VALUE_0=https://github.com/
RUN apt-get update && apt-get install -y --no-install-recommends webkit2gtk-driver dbus-x11 python3 \
  && rm -rf /var/lib/apt/lists/* && cargo install tauri-driver --locked
