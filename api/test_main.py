"""
Test suite for PanDiff FastAPI application.
"""

import os
import tempfile
import zipfile
import pytest
from fastapi.testclient import TestClient
from unittest.mock import patch, MagicMock
import io

# Set environment variables before importing the app
os.environ["BEARER_TOKEN"] = "test-token-123"
os.environ["PANDIFF_CMD"] = "pandiff"

from main import app

client = TestClient(app)


@pytest.fixture
def auth_headers():
    """Headers with valid authentication token"""
    return {"Authorization": "Bearer test-token-123"}


@pytest.fixture
def invalid_auth_headers():
    """Headers with invalid authentication token"""
    return {"Authorization": "Bearer invalid-token"}


@pytest.fixture
def sample_markdown_file1():
    """Sample markdown content for testing"""
    return b"""# Document 1

This is the first document with some content.

- Item 1
- Item 2
- Item 3
"""


@pytest.fixture
def sample_markdown_file2():
    """Sample markdown content for testing (modified version)"""
    return b"""# Document 1 Updated

This is the first document with some modified content.

- Item 1
- Item 2 (updated)
- Item 3
- Item 4 (new)
"""


def test_root_endpoint():
    """Test the root endpoint"""
    response = client.get("/")
    assert response.status_code == 200
    data = response.json()
    assert data["message"] == "PanDiff API"
    assert data["version"] == "1.0.0"
    assert "/compare" in data["endpoints"]
    assert "/compare-bidirectional" in data["endpoints"]


def test_health_check_with_pandiff_available():
    """Test health check when pandiff is available"""
    with patch("subprocess.run") as mock_run:
        mock_run.return_value.returncode = 0
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "healthy"
        assert data["pandiff_available"] is True


def test_health_check_with_pandiff_unavailable():
    """Test health check when pandiff is not available"""
    with patch("subprocess.run") as mock_run:
        mock_run.side_effect = FileNotFoundError()
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "unhealthy"
        assert data["pandiff_available"] is False


def test_compare_without_auth():
    """Test compare endpoint without authentication"""
    response = client.post(
        "/compare",
        files={
            "document1": ("test1.md", b"# Test 1", "text/markdown"),
            "document2": ("test2.md", b"# Test 2", "text/markdown"),
        }
    )
    assert response.status_code == 403


def test_compare_with_invalid_auth(invalid_auth_headers):
    """Test compare endpoint with invalid authentication"""
    response = client.post(
        "/compare",
        headers=invalid_auth_headers,
        files={
            "document1": ("test1.md", b"# Test 1", "text/markdown"),
            "document2": ("test2.md", b"# Test 2", "text/markdown"),
        }
    )
    assert response.status_code == 401


def test_compare_invalid_output_format(auth_headers):
    """Test compare endpoint with invalid output format"""
    response = client.post(
        "/compare",
        headers=auth_headers,
        files={
            "document1": ("test1.md", b"# Test 1", "text/markdown"),
            "document2": ("test2.md", b"# Test 2", "text/markdown"),
        },
        params={"output_format": "invalid"}
    )
    assert response.status_code == 400


@patch("main.run_pandiff")
def test_compare_success(mock_run_pandiff, auth_headers, sample_markdown_file1, sample_markdown_file2):
    """Test successful document comparison"""
    # Mock the pandiff output
    mock_output = b"<html><body><h1>Diff Result</h1></body></html>"
    mock_run_pandiff.return_value = mock_output
    
    response = client.post(
        "/compare",
        headers=auth_headers,
        files={
            "document1": ("doc1.md", sample_markdown_file1, "text/markdown"),
            "document2": ("doc2.md", sample_markdown_file2, "text/markdown"),
        },
        params={"output_format": "html"}
    )
    
    assert response.status_code == 200
    assert response.headers["content-type"] == "text/html; charset=utf-8"
    assert "Content-Disposition" in response.headers
    assert "diff.html" in response.headers["Content-Disposition"]
    assert response.content == mock_output


@patch("main.run_pandiff")
def test_compare_pdf_format(mock_run_pandiff, auth_headers, sample_markdown_file1, sample_markdown_file2):
    """Test document comparison with PDF output"""
    mock_output = b"%PDF-1.4 fake pdf content"
    mock_run_pandiff.return_value = mock_output
    
    response = client.post(
        "/compare",
        headers=auth_headers,
        files={
            "document1": ("doc1.md", sample_markdown_file1, "text/markdown"),
            "document2": ("doc2.md", sample_markdown_file2, "text/markdown"),
        },
        params={"output_format": "pdf"}
    )
    
    assert response.status_code == 200
    assert response.headers["content-type"] == "application/pdf"
    assert "diff.pdf" in response.headers["Content-Disposition"]


def test_compare_bidirectional_without_auth():
    """Test bidirectional compare endpoint without authentication"""
    response = client.post(
        "/compare-bidirectional",
        files={
            "document1": ("test1.md", b"# Test 1", "text/markdown"),
            "document2": ("test2.md", b"# Test 2", "text/markdown"),
        }
    )
    assert response.status_code == 403


@patch("main.run_pandiff")
def test_compare_bidirectional_success(mock_run_pandiff, auth_headers, sample_markdown_file1, sample_markdown_file2):
    """Test successful bidirectional document comparison"""
    # Mock the pandiff outputs
    mock_output_1_vs_2 = b"<html><body><h1>Doc1 vs Doc2</h1></body></html>"
    mock_output_2_vs_1 = b"<html><body><h1>Doc2 vs Doc1</h1></body></html>"
    
    # Configure mock to return different outputs based on call order
    mock_run_pandiff.side_effect = [mock_output_1_vs_2, mock_output_2_vs_1]
    
    response = client.post(
        "/compare-bidirectional",
        headers=auth_headers,
        files={
            "document1": ("doc1.md", sample_markdown_file1, "text/markdown"),
            "document2": ("doc2.md", sample_markdown_file2, "text/markdown"),
        },
        params={"output_format": "html"}
    )
    
    assert response.status_code == 200
    assert response.headers["content-type"] == "application/zip"
    assert "bidirectional_diff.zip" in response.headers["Content-Disposition"]
    
    # Verify ZIP file contents
    zip_buffer = io.BytesIO(response.content)
    with zipfile.ZipFile(zip_buffer, 'r') as zip_file:
        file_list = zip_file.namelist()
        assert len(file_list) == 2
        assert "doc1_vs_doc2.html" in file_list
        assert "doc2_vs_doc1.html" in file_list
        
        # Check content of each file
        assert zip_file.read("doc1_vs_doc2.html") == mock_output_1_vs_2
        assert zip_file.read("doc2_vs_doc1.html") == mock_output_2_vs_1


@patch("main.subprocess.run")
def test_pandiff_command_failure(mock_subprocess, auth_headers, sample_markdown_file1, sample_markdown_file2):
    """Test handling of pandiff command failure"""
    # Mock subprocess to raise CalledProcessError
    from subprocess import CalledProcessError
    mock_subprocess.side_effect = CalledProcessError(1, "pandiff", stderr="Error message")
    
    response = client.post(
        "/compare",
        headers=auth_headers,
        files={
            "document1": ("doc1.md", sample_markdown_file1, "text/markdown"),
            "document2": ("doc2.md", sample_markdown_file2, "text/markdown"),
        }
    )
    
    assert response.status_code == 500
    assert "Pandiff command failed" in response.json()["detail"]


@patch("main.subprocess.run")
def test_pandiff_command_not_found(mock_subprocess, auth_headers, sample_markdown_file1, sample_markdown_file2):
    """Test handling when pandiff command is not found"""
    mock_subprocess.side_effect = FileNotFoundError()
    
    response = client.post(
        "/compare",
        headers=auth_headers,
        files={
            "document1": ("doc1.md", sample_markdown_file1, "text/markdown"),
            "document2": ("doc2.md", sample_markdown_file2, "text/markdown"),
        }
    )
    
    assert response.status_code == 500
    assert "Pandiff command not found" in response.json()["detail"]


def test_compare_bidirectional_invalid_format(auth_headers):
    """Test bidirectional compare with invalid output format"""
    response = client.post(
        "/compare-bidirectional",
        headers=auth_headers,
        files={
            "document1": ("test1.md", b"# Test 1", "text/markdown"),
            "document2": ("test2.md", b"# Test 2", "text/markdown"),
        },
        params={"output_format": "invalid"}
    )
    assert response.status_code == 400


if __name__ == "__main__":
    pytest.main([__file__, "-v"])