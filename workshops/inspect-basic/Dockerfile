FROM quay.io/jupyter/base-notebook:hub-5.4.1

# Switch to root for system packages
USER root

# Install additional Python packages
RUN apt-get update && apt-get install -y \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

# Install code-server
RUN curl -fsSL https://code-server.dev/install.sh | sh

# Switch back to jovyan user (JupyterHub default)
USER ${NB_USER}

# Install JupyterHub and proxy extensions
RUN pip install --no-cache-dir \
    jupyter-server-proxy \
    inspect-ai \
    openai \
    anthropic \
    google-generativeai
RUN jupyter serverextension enable --sys-prefix jupyter_server_proxy

# Install VS Code extensions
RUN code-server --install-extension ms-python.python && \
    code-server --install-extension ukaisi.inspect-ai

# Create workspace directory
RUN mkdir -p /home/${NB_USER}/workspace

# Copy workspace files
COPY --chown=${NB_UID}:${NB_GID} ./workspace /home/${NB_USER}/workspace

# Copy requirements if you have additional ones
COPY --chown=${NB_UID}:${NB_GID} requirements.txt /tmp/
RUN pip install --no-cache-dir -r /tmp/requirements.txt

WORKDIR /home/${NB_USER}

# The default command will be overridden by JupyterHub
CMD ["start-notebook.sh"]