# PanDiff API

A FastAPI web service that provides REST endpoints for comparing documents using the PanDiff tool. This API allows you to upload two documents and receive redlines showing the differences between them.

## Features

- **Bearer Token Authentication**: Secure access using configurable bearer tokens
- **Multiple Output Formats**: Support for HTML, PDF, DOCX, and Markdown outputs
- **Single Document Comparison**: Compare two documents and get a redline
- **Bidirectional Comparison**: Get a ZIP file with comparisons in both directions
- **File Upload Support**: Accept various document formats supported by Pandoc
- **Docker Support**: Easy deployment with Docker and Docker Compose

## API Endpoints

### `GET /`
Returns API information and available endpoints.

### `GET /health` 
Health check endpoint that verifies pandiff availability.

### `POST /compare`
Compare two documents and return a redline showing differences.

**Parameters:**
- `document1` (file): First document to compare
- `document2` (file): Second document to compare  
- `output_format` (query, optional): Output format (`html`, `pdf`, `docx`, `md`) - defaults to `html`

**Headers:**
- `Authorization: Bearer <token>`: Required authentication token

**Response:** 
- Single file with the comparison result

### `POST /compare-bidirectional`
Compare two documents bidirectionally and return a ZIP file containing both comparisons.

**Parameters:**
- `document1` (file): First document to compare
- `document2` (file): Second document to compare
- `output_format` (query, optional): Output format (`html`, `pdf`, `docx`, `md`) - defaults to `html`

**Headers:**
- `Authorization: Bearer <token>`: Required authentication token

**Response:**
- ZIP file containing:
  - `doc1_vs_doc2.{format}`: Changes from document1 to document2
  - `doc2_vs_doc1.{format}`: Changes from document2 to document1

## Installation & Setup

### Using Docker (Recommended)

1. **Build and run with Docker Compose:**
   ```bash
   cd api
   docker-compose up --build
   ```

2. **Or build and run manually:**
   ```bash
   # Build from the pandiff project root
   docker build -f api/Dockerfile -t pandiff-api .
   
   # Run the container
   docker run -p 8000:8000 -e BEARER_TOKEN=REDACTED pandiff-api
   ```

### Local Development

1. **Prerequisites:**
   - Python 3.11+
   - Node.js 18+ 
   - Pandoc installed and in PATH
   - LaTeX distribution (for PDF output)

2. **Install pandiff globally:**
   ```bash
   # From the project root
   npm install -g .
   ```

3. **Set up Python environment:**
   ```bash
   cd api
   python -m venv venv
   source venv/bin/activate  # On Windows: venv\Scripts\activate
   pip install -r requirements.txt
   ```

4. **Configure environment:**
   ```bash
   cp .env.example .env
   # Edit .env with your settings
   ```

5. **Run the API:**
   ```bash
   python -m uvicorn main:app --host 0.0.0.0 --port 8000 --reload
   ```

## Configuration

Set these environment variables:

- `BEARER_TOKEN`: Authentication token for API access (required)
- `PANDIFF_CMD`: Path to pandiff command (default: `pandiff`) 
- `PORT`: Port for the API server (default: `8000`)

## Testing

Run the test suite:

```bash
cd api
pytest test_main.py -v
```

## Usage Examples

### Using cURL

1. **Basic comparison (HTML output):**
   ```bash
   curl -X POST "http://localhost:8000/compare?output_format=html" \
     -H "Authorization: Bearer REDACTED" \
     -F "document1=@old.md" \
     -F "document2=@new.md" \
     -o diff.html
   ```

2. **PDF comparison:**
   ```bash
   curl -X POST "http://localhost:8000/compare?output_format=pdf" \
     -H "Authorization: Bearer REDACTED" \
     -F "document1=@old.docx" \
     -F "document2=@new.docx" \
     -o diff.pdf
   ```

3. **Bidirectional comparison:**
   ```bash
   curl -X POST "http://localhost:8000/compare-bidirectional?output_format=html" \
     -H "Authorization: Bearer REDACTED" \
     -F "document1=@version1.md" \
     -F "document2=@version2.md" \
     -o bidirectional_diff.zip
   ```

### Using Python requests

```python
import requests

url = "http://localhost:8000/compare"
headers = {"Authorization": "Bearer REDACTED"}

with open("doc1.md", "rb") as f1, open("doc2.md", "rb") as f2:
    files = {
        "document1": ("doc1.md", f1, "text/markdown"),
        "document2": ("doc2.md", f2, "text/markdown")
    }
    params = {"output_format": "html"}
    
    response = requests.post(url, headers=headers, files=files, params=params)
    
    if response.status_code == 200:
        with open("diff.html", "wb") as output:
            output.write(response.content)
    else:
        print(f"Error: {response.status_code} - {response.text}")
```

### Interactive API Documentation

Once the service is running, visit:
- Swagger UI: `http://localhost:8000/docs`
- ReDoc: `http://localhost:8000/redoc`

## Supported Document Formats

The API supports any document format that Pandoc can process, including:
- Markdown (.md)
- Microsoft Word (.docx) 
- LaTeX (.tex)
- HTML (.html)
- reStructuredText (.rst)
- Org-mode (.org)
- EPUB (.epub)
- And many more...

## Security Considerations

- Always use strong, unique bearer tokens in production
- Consider rate limiting for public deployments
- Temporary files are automatically cleaned up
- The service runs with standard user permissions (not root)

## Troubleshooting

1. **"Pandiff command not found" error:**
   - Ensure pandiff is installed and in PATH
   - For Docker: this should be handled automatically

2. **Authentication failures:**
   - Verify the `Authorization: Bearer <token>` header is set correctly
   - Check that `BEARER_TOKEN` environment variable matches

3. **PDF generation issues:**
   - Ensure LaTeX is installed (included in Docker image)
   - Check pandoc version supports PDF output

4. **File upload errors:**
   - Verify files are not empty
   - Check file format is supported by Pandoc
   - Ensure proper Content-Type headers