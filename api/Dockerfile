# Multi-stage Dockerfile for PanDiff API
# Stage 1: Build the pandiff Node.js application
FROM node:18-bullseye as pandiff-builder

# Install system dependencies including Pandoc
RUN apt-get update && apt-get install -y \
    pandoc \
    texlive-latex-base \
    texlive-latex-recommended \
    texlive-latex-extra \
    && rm -rf /var/lib/apt/lists/*

# Set working directory for pandiff build
WORKDIR /pandiff

# Copy package files
COPY package.json ./
COPY src/ ./src/
COPY tsconfig.json ./
COPY assets/ ./assets/

# Install dependencies and build
RUN npm install
RUN npm run compile

# Stage 2: Create the Python FastAPI runtime
FROM python:3.11-bullseye

# Install system dependencies including Pandoc and Node.js setup
RUN apt-get update && apt-get install -y \
    pandoc \
    texlive-latex-base \
    texlive-latex-recommended \
    texlive-latex-extra \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 18.x
RUN curl -fsSL https://deb.nodesource.com/setup_18.x | bash - \
    && apt-get install -y nodejs

# Install yarn globally
RUN npm install -g yarn

# Set working directory
WORKDIR /app

# Copy requirements and install Python dependencies
COPY api/requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy the built pandiff from stage 1 (all needed files)
COPY --from=pandiff-builder /pandiff/lib /pandiff/lib
COPY --from=pandiff-builder /pandiff/assets /pandiff/assets
COPY --from=pandiff-builder /pandiff/node_modules /pandiff/node_modules
COPY --from=pandiff-builder /pandiff/package.json /pandiff/package.json

# Make pandiff available in PATH
RUN chmod +x /pandiff/lib/cli.js
RUN ln -s /pandiff/lib/cli.js /usr/local/bin/pandiff

# Copy Python API application
WORKDIR /app
COPY api/main.py .
COPY api/test_main.py .

# Set environment variables
ENV PANDIFF_CMD=pandiff
ENV BEARER_TOKEN=your-secret-token-here
ENV PORT=8000

# Expose port
EXPOSE 8000

# Health check
HEALTHCHECK --interval=30s --timeout=30s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8000/health || exit 1

# Run the application
CMD ["python", "-m", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]