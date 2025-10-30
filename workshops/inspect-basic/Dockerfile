FROM codercom/code-server:latest

# Install Python and pip
USER root
RUN apt-get update && apt-get install -y \
    python3 \
    python3-pip \
    python3-venv \
    && rm -rf /var/lib/apt/lists/*

# Create workspace directory
RUN mkdir -p /home/coder/workspace

# Switch to coder user
USER coder

# Install Python extension
RUN code-server --install-extension ms-python.python

# Install inspect-ai extension (adjust if you have a specific extension ID)
RUN code-server --install-extension ukaisi.inspect-ai

# Copy your workspace files
# COPY --chown=coder:coder ./your-workspace /home/coder/workspace

# Configure code-server settings
RUN mkdir -p /home/coder/.local/share/code-server/User
RUN echo '{\n\
  "workbench.startupEditor": "none",\n\
  "window.menuBarVisibility": "toggle"\n\
}' > /home/coder/.local/share/code-server/User/settings.json

# Set workspace as working directory
WORKDIR /home/coder/workspace

# Expose code-server port
EXPOSE 8080

# Start code-server without authentication, opening the workspace
CMD ["code-server", "--bind-addr", "0.0.0.0:8080", "--auth", "none", "/home/coder/workspace"]