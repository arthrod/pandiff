#!/usr/bin/env python3
"""
Manual test script for the PanDiff API endpoints.
This script tests the API by calling the functions directly rather than over HTTP.
"""

import tempfile
import os
from main import app, save_temp_file, run_pandiff

# Test the save_temp_file function
def test_save_temp_file():
    print("Testing save_temp_file...")
    content = b"Hello, World!"
    path = save_temp_file(content, ".txt")
    assert os.path.exists(path)
    with open(path, 'rb') as f:
        assert f.read() == content
    os.unlink(path)
    print("✓ save_temp_file works correctly")

# Test the pandiff command
def test_pandiff_command():
    print("Testing run_pandiff...")
    try:
        # Create two simple test files
        doc1_content = b"# Test Doc 1\nThis is the first document."
        doc2_content = b"# Test Doc 1 Updated\nThis is the first document with changes."
        
        doc1_path = save_temp_file(doc1_content, ".md")
        doc2_path = save_temp_file(doc2_content, ".md")
        
        try:
            result = run_pandiff(doc1_path, doc2_path, "html")
            assert len(result) > 0
            assert b"<" in result and b">" in result  # Should contain HTML
            print("✓ run_pandiff works correctly")
        finally:
            os.unlink(doc1_path)
            os.unlink(doc2_path)
            
    except Exception as e:
        print(f"✗ run_pandiff failed: {e}")

if __name__ == "__main__":
    print("Running manual API tests...")
    test_save_temp_file()
    test_pandiff_command()
    print("Manual tests completed!")