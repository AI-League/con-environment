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
RUN code-server --install-extension janisdd.vscode-edit-csv

# Copy and install your specific requirements FIRST
COPY --chown=coder:coder requirements.txt /home/coder/
RUN pip install --no-cache-dir -r /home/coder/requirements.txt

# Copy download script and run it
COPY --chown=coder:coder ./workspace /home/coder/workspace

WORKDIR /home/coder/workspace

# Create a startup script that activates venv before running code-server
COPY --chown=coder:coder ./start.sh /home/coder/start.sh


RUN chmod +x /home/coder/start.sh

CMD ["/home/coder/start.sh"]