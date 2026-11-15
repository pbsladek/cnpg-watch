"""Bounded redaction for isolated-fixture artifacts; never copy kubeconfig/Secrets."""
import re
import sys
from pathlib import Path
text = sys.stdin.buffer.read(65537).decode("utf-8", errors="replace")
for filename in sys.argv[1:]:
    password = Path(filename).read_text().strip() if Path(filename).exists() else ""
    if password:
        text = text.replace(password, "[REDACTED]")
text = re.sub(r"(?i)(PASSWORD\s+)(?:E)?'(?:''|[^'])*'", r"\1'[REDACTED]'", text)
text = re.sub(r"(?i)(?:postgres(?:ql)?://|bearer\s+)\S+", "[REDACTED]", text)
text = re.sub(r"(?i)(password|token)\s*[:=]\s*[^\s,;]+", r"\1=[REDACTED]", text)
text = re.sub(r"\b(?:\d{1,3}\.){3}\d{1,3}(?::\d+)?\b", "[address]", text)
text = re.sub(r'(?i)("(?:query|statement)"\s*:\s*)"(?:[^"\\]|\\.)*"', r'\1"[REDACTED]"', text)
if len(text) > 65536:
    text = text[:65536] + "\n[truncated]\n"
sys.stdout.write(text)
