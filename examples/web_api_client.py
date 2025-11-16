#!/usr/bin/env python3
"""
Example Python client for slopdrop Web API
Demonstrates how to interact with slopdrop programmatically

Usage:
    python3 examples/web_api_client.py
"""

import requests
import json
import sys

class SlopdropClient:
    def __init__(self, base_url="http://127.0.0.1:8080"):
        self.base_url = base_url
        self.session = requests.Session()

    def eval(self, code, is_admin=False):
        """Evaluate TCL code"""
        response = self.session.post(
            f"{self.base_url}/api/eval",
            json={
                "code": code,
                "is_admin": is_admin
            }
        )
        response.raise_for_status()
        return response.json()

    def more(self):
        """Get more paginated output"""
        response = self.session.get(f"{self.base_url}/api/more")
        response.raise_for_status()
        return response.json()

    def history(self, limit=10):
        """Get git commit history"""
        response = self.session.get(
            f"{self.base_url}/api/history",
            params={"limit": limit}
        )
        response.raise_for_status()
        return response.json()

    def rollback(self, commit_hash):
        """Rollback to specific commit"""
        response = self.session.post(
            f"{self.base_url}/api/rollback",
            json={"commit_hash": commit_hash}
        )
        response.raise_for_status()
        return response.json()

    def health(self):
        """Check server health"""
        response = self.session.get(f"{self.base_url}/api/health")
        response.raise_for_status()
        return response.json()


def main():
    # Create client
    client = SlopdropClient()

    # Check health
    print("Checking server health...")
    health = client.health()
    print(f"✓ Server is {health['status']}\n")

    # Simple evaluation
    print("Evaluating: expr {1 + 1}")
    result = client.eval("expr {1 + 1}")
    print(f"Result: {result['output']}")
    print(f"Error: {result['is_error']}\n")

    # Set a variable
    print("Evaluating: set myvar \"Hello from Python!\"")
    result = client.eval("set myvar \"Hello from Python!\"")
    print(f"Result: {result['output']}\n")

    # Read the variable
    print("Evaluating: set myvar")
    result = client.eval("set myvar")
    print(f"Result: {result['output']}\n")

    # Define a procedure
    print("Defining factorial procedure...")
    factorial_code = """
proc factorial {n} {
    if {$n <= 1} {
        return 1
    }
    expr {$n * [factorial [expr {$n - 1}]]}
}
    """.strip()
    result = client.eval(factorial_code, is_admin=True)
    print(f"Result: {result['output']}\n")

    # Call the procedure
    print("Evaluating: factorial 5")
    result = client.eval("factorial 5")
    print(f"Result: {result['output']}\n")

    # Get history
    print("Getting git history...")
    history = client.history(5)
    print("Recent commits:")
    for commit in history['history']:
        print(f"  {commit['commit_id'][:8]} - {commit['author']} - {commit['message']}")
    print()

    # Test pagination with large output
    print("Testing pagination with large output...")
    result = client.eval("for {set i 0} {$i < 50} {incr i} { puts \"Line $i\" }")
    print(f"Got {len(result['output'])} lines")
    print(f"More available: {result['more_available']}")

    if result['more_available']:
        print("\nGetting more output...")
        more_result = client.more()
        print(f"Got {len(more_result['output'])} more lines")
        print(f"Still more available: {more_result['more_available']}")

    print("\n✓ All examples completed successfully!")


if __name__ == "__main__":
    try:
        main()
    except requests.exceptions.ConnectionError:
        print("Error: Cannot connect to slopdrop server", file=sys.stderr)
        print("Make sure to start the server first:", file=sys.stderr)
        print("  ./target/release/slopdrop --web", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
