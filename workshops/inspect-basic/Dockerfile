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

# Create venv and install ONLY the minimal JupyterHub requirements
RUN python3 -m venv /home/coder/venv
ENV PATH="/home/coder/venv/bin:$PATH"

# Install VS Code extensions
RUN code-server --install-extension ms-python.python
RUN code-server --install-extension ukaisi.inspect-ai

# Copy your workspace files
COPY --chown=coder:coder ./workspace /home/coder/workspace

# Copy and install your specific requirements
COPY --chown=coder:coder requirements.txt /home/coder/
RUN pip install --no-cache-dir -r /home/coder/requirements.txt

WORKDIR /home/coder/workspace

# This is the key - run jupyterhub-singleuser which handles the auth and API
CMD ["code-server","--bind-addr", "0.0.0.0:8888","--auth", "none","--disable-telemetry","/home/coder/workspace"]