"""
FastAPI application for PanDiff document comparison service.

This service provides REST endpoints for comparing documents and generating redlines
using the pandiff tool. It supports bearer token authentication and can return
single redlines or zip files with bidirectional comparisons.
"""

import os
import tempfile
import subprocess
import zipfile
import io
from typing import Annotated, List
from pathlib import Path

from fastapi import FastAPI, HTTPException, Depends, UploadFile, File, status
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from fastapi.responses import Response, StreamingResponse
from pydantic import BaseModel


# Configuration
BEARER_TOKEN = os.getenv("BEARER_TOKEN", "your-secret-token-here")
PANDIFF_CMD = os.getenv("PANDIFF_CMD", "pandiff")

app = FastAPI(
    title="PanDiff API",
    description="Document comparison service using pandiff",
    version="1.0.0"
)

security = HTTPBearer()


class DocumentComparisonRequest(BaseModel):
    """Request model for document comparison with base64 encoded content"""
    document1_name: str
    document1_content: str  # base64 encoded
    document2_name: str
    document2_content: str  # base64 encoded
    output_format: str = "html"  # html, pdf, docx, md


def verify_token(credentials: HTTPAuthorizationCredentials = Depends(security)):
    """Verify the bearer token"""
    if credentials.credentials != BEARER_TOKEN:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid authentication token",
            headers={"WWW-Authenticate": "Bearer"},
        )
    return credentials.credentials


def save_temp_file(content: bytes, suffix: str = "") -> str:
    """Save content to a temporary file and return the path"""
    with tempfile.NamedTemporaryFile(mode='wb', delete=False, suffix=suffix) as f:
        f.write(content)
        return f.name


def run_pandiff(file1_path: str, file2_path: str, output_format: str = "html") -> bytes:
    """Run pandiff command and return the output"""
    with tempfile.NamedTemporaryFile(suffix=f".{output_format}", delete=False) as output_file:
        try:
            cmd = [PANDIFF_CMD, file1_path, file2_path, "-o", output_file.name]
            if output_format == "html":
                cmd.extend(["-s"])  # standalone HTML
            
            result = subprocess.run(cmd, capture_output=True, text=True, check=True)
            
            # Read the output file
            if os.path.exists(output_file.name):
                with open(output_file.name, 'rb') as f:
                    content = f.read()
                    os.unlink(output_file.name)  # Clean up
                    return content
            else:
                return b""
                
        except subprocess.CalledProcessError as e:
            if os.path.exists(output_file.name):
                os.unlink(output_file.name)
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail=f"Pandiff command failed: {e.stderr}"
            )
        except FileNotFoundError as e:
            if os.path.exists(output_file.name):
                os.unlink(output_file.name)
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Pandiff command not found. Make sure pandiff is installed and in PATH."
            )


def get_content_type(output_format: str) -> str:
    """Get the appropriate content type for the output format"""
    content_types = {
        "html": "text/html",
        "pdf": "application/pdf",
        "docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "md": "text/markdown"
    }
    return content_types.get(output_format, "application/octet-stream")


@app.get("/")
async def root():
    """Root endpoint with API information"""
    return {
        "message": "PanDiff API",
        "version": "1.0.0",
        "endpoints": {
            "/compare": "POST - Compare two documents and return redline",
            "/compare-bidirectional": "POST - Compare two documents and return zip with bidirectional redlines"
        }
    }


@app.get("/health")
async def health_check():
    """Health check endpoint"""
    try:
        # Test if pandiff is available
        result = subprocess.run([PANDIFF_CMD, "--version"], capture_output=True, text=True)
        pandiff_available = result.returncode == 0
    except FileNotFoundError:
        pandiff_available = False
    
    return {
        "status": "healthy" if pandiff_available else "unhealthy",
        "pandiff_available": pandiff_available
    }


@app.post("/compare")
async def compare_documents(
    document1: UploadFile = File(...),
    document2: UploadFile = File(...),
    output_format: str = "html",
    token: str = Depends(verify_token)
):
    """
    Compare two documents and return a redline showing the differences.
    
    - **document1**: First document file
    - **document2**: Second document file  
    - **output_format**: Output format (html, pdf, docx, md)
    """
    
    # Validate output format
    if output_format not in ["html", "pdf", "docx", "md"]:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Invalid output format. Supported: html, pdf, docx, md"
        )
    
    temp_files = []
    try:
        # Save uploaded files to temporary files
        doc1_content = await document1.read()
        doc2_content = await document2.read()
        
        doc1_suffix = Path(document1.filename or "").suffix
        doc2_suffix = Path(document2.filename or "").suffix
        
        doc1_path = save_temp_file(doc1_content, doc1_suffix)
        doc2_path = save_temp_file(doc2_content, doc2_suffix)
        temp_files.extend([doc1_path, doc2_path])
        
        # Run pandiff
        result = run_pandiff(doc1_path, doc2_path, output_format)
        
        # Return the result
        return Response(
            content=result,
            media_type=get_content_type(output_format),
            headers={
                "Content-Disposition": f"attachment; filename=diff.{output_format}"
            }
        )
        
    finally:
        # Clean up temporary files
        for temp_file in temp_files:
            try:
                os.unlink(temp_file)
            except OSError:
                pass


@app.post("/compare-bidirectional")
async def compare_documents_bidirectional(
    document1: UploadFile = File(...),
    document2: UploadFile = File(...),
    output_format: str = "html",
    token: str = Depends(verify_token)
):
    """
    Compare two documents bidirectionally and return a zip file containing:
    - doc1_vs_doc2: Changes from document1 to document2
    - doc2_vs_doc1: Changes from document2 to document1
    
    - **document1**: First document file
    - **document2**: Second document file
    - **output_format**: Output format for both comparisons (html, pdf, docx, md)
    """
    
    # Validate output format
    if output_format not in ["html", "pdf", "docx", "md"]:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Invalid output format. Supported: html, pdf, docx, md"
        )
    
    temp_files = []
    try:
        # Save uploaded files to temporary files
        doc1_content = await document1.read()
        doc2_content = await document2.read()
        
        doc1_suffix = Path(document1.filename or "doc1").suffix
        doc2_suffix = Path(document2.filename or "doc2").suffix
        
        doc1_path = save_temp_file(doc1_content, doc1_suffix)
        doc2_path = save_temp_file(doc2_content, doc2_suffix)
        temp_files.extend([doc1_path, doc2_path])
        
        # Generate both comparisons
        result1_vs_2 = run_pandiff(doc1_path, doc2_path, output_format)  # doc1 -> doc2
        result2_vs_1 = run_pandiff(doc2_path, doc1_path, output_format)  # doc2 -> doc1
        
        # Create ZIP file in memory
        zip_buffer = io.BytesIO()
        
        with zipfile.ZipFile(zip_buffer, 'w', zipfile.ZIP_DEFLATED) as zip_file:
            # Add first comparison (doc1 vs doc2)
            doc1_name = Path(document1.filename or "document1").stem
            doc2_name = Path(document2.filename or "document2").stem
            
            zip_file.writestr(
                f"{doc1_name}_vs_{doc2_name}.{output_format}",
                result1_vs_2
            )
            
            # Add second comparison (doc2 vs doc1)
            zip_file.writestr(
                f"{doc2_name}_vs_{doc1_name}.{output_format}",
                result2_vs_1
            )
        
        zip_buffer.seek(0)
        
        return StreamingResponse(
            io.BytesIO(zip_buffer.read()),
            media_type="application/zip",
            headers={
                "Content-Disposition": "attachment; filename=bidirectional_diff.zip"
            }
        )
        
    finally:
        # Clean up temporary files
        for temp_file in temp_files:
            try:
                os.unlink(temp_file)
            except OSError:
                pass


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        app, 
        host="0.0.0.0", 
        port=int(os.getenv("PORT", "8000"))
    )