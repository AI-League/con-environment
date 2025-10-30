FROM codercom/code-server:latest

# Install Python and pip
USER root
RUN apt-get update && apt-get install -y \
    python3 \
    python3-pip \
    python3-venv \
    python3-full \
    && rm -rf /var/lib/apt/lists/*

# Create workspace directory
RUN mkdir -p /home/coder/workspace
RUN chown -R coder:coder /home/coder/workspace
# Switch to coder user
USER coder



RUN python3 -m venv /home/coder/venv
ENV PATH="/home/coder/venv/bin:$PATH"

RUN code-server --install-extension ms-python.python
RUN code-server --install-extension ukaisi.inspect-ai

# Copy your workspace files
COPY --chown=coder:coder ./workspace /home/coder/workspace

# Configure code-server settings
RUN mkdir -p /home/coder/.local/share/code-server/User
RUN echo '{\n\
  "workbench.startupEditor": "none",\n\
  "window.menuBarVisibility": "toggle",\n\
  "python.defaultInterpreterPath": "/home/coder/venv/bin/python"\n\
}' > /home/coder/.local/share/code-server/User/settings.json

WORKDIR /home/coder/workspace

COPY requirements.txt /home/coder/
RUN pip install -r /home/coder/requirements.txt

EXPOSE 8080
CMD ["code-server", "--bind-addr", "0.0.0.0:8080", "--auth", "none", "/home/coder/workspace"]