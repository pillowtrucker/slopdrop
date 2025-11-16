#!/bin/bash
# Example curl commands for slopdrop Web API
# Demonstrates REST API usage with curl

BASE_URL="http://127.0.0.1:8080"

echo "=== Slopdrop Web API Examples with curl ==="
echo ""

# Health check
echo "1. Health Check"
echo "   GET /api/health"
curl -s "${BASE_URL}/api/health" | jq '.'
echo ""
echo ""

# Simple evaluation
echo "2. Simple Evaluation"
echo "   POST /api/eval"
echo "   Body: {\"code\":\"expr {1 + 1}\",\"is_admin\":false}"
curl -s -X POST "${BASE_URL}/api/eval" \
  -H 'Content-Type: application/json' \
  -d '{"code":"expr {1 + 1}","is_admin":false}' | jq '.'
echo ""
echo ""

# Set a variable
echo "3. Set Variable"
echo "   POST /api/eval"
echo "   Body: {\"code\":\"set greeting \\\"Hello, API!\\\"\",\"is_admin\":true}"
curl -s -X POST "${BASE_URL}/api/eval" \
  -H 'Content-Type: application/json' \
  -d '{"code":"set greeting \"Hello, API!\"","is_admin":true}' | jq '.'
echo ""
echo ""

# Read the variable
echo "4. Read Variable"
echo "   POST /api/eval"
echo "   Body: {\"code\":\"set greeting\",\"is_admin\":false}"
curl -s -X POST "${BASE_URL}/api/eval" \
  -H 'Content-Type: application/json' \
  -d '{"code":"set greeting","is_admin":false}' | jq '.'
echo ""
echo ""

# Define a procedure (admin only)
echo "5. Define Procedure (requires admin)"
echo "   POST /api/eval"
PROC_CODE='proc double {x} { expr {$x * 2} }'
curl -s -X POST "${BASE_URL}/api/eval" \
  -H 'Content-Type: application/json' \
  -d "{\"code\":\"${PROC_CODE}\",\"is_admin\":true}" | jq '.'
echo ""
echo ""

# Call the procedure
echo "6. Call Procedure"
echo "   POST /api/eval"
echo "   Body: {\"code\":\"double 21\",\"is_admin\":false}"
curl -s -X POST "${BASE_URL}/api/eval" \
  -H 'Content-Type: application/json' \
  -d '{"code":"double 21","is_admin":false}' | jq '.'
echo ""
echo ""

# List operations
echo "7. List Operations"
echo "   POST /api/eval"
curl -s -X POST "${BASE_URL}/api/eval" \
  -H 'Content-Type: application/json' \
  -d '{"code":"set mylist [list 1 2 3 4 5]; lappend mylist 6; llength $mylist","is_admin":false}' | jq '.'
echo ""
echo ""

# String operations
echo "8. String Operations"
echo "   POST /api/eval"
curl -s -X POST "${BASE_URL}/api/eval" \
  -H 'Content-Type: application/json' \
  -d '{"code":"string toupper \"hello world\"","is_admin":false}' | jq '.'
echo ""
echo ""

# Generate large output to test pagination
echo "9. Pagination Test (large output)"
echo "   POST /api/eval"
LOOP_CODE='for {set i 0} {$i < 50} {incr i} { puts "Line $i" }'
RESPONSE=$(curl -s -X POST "${BASE_URL}/api/eval" \
  -H 'Content-Type: application/json' \
  -d "{\"code\":\"${LOOP_CODE}\",\"is_admin\":false}")
echo "$RESPONSE" | jq '.'
MORE_AVAILABLE=$(echo "$RESPONSE" | jq -r '.more_available')
echo ""

if [ "$MORE_AVAILABLE" = "true" ]; then
    echo "10. Get More Output"
    echo "    GET /api/more"
    curl -s "${BASE_URL}/api/more" | jq '.'
    echo ""
    echo ""
fi

# Get git history
echo "11. Git History"
echo "    GET /api/history?limit=5"
curl -s "${BASE_URL}/api/history?limit=5" | jq '.'
echo ""
echo ""

# Error handling example
echo "12. Error Handling (invalid TCL)"
echo "    POST /api/eval"
curl -s -X POST "${BASE_URL}/api/eval" \
  -H 'Content-Type: application/json' \
  -d '{"code":"invalid tcl syntax {{{","is_admin":false}' | jq '.'
echo ""
echo ""

echo "=== Examples Complete ==="
echo ""
echo "Note: Some commands require 'jq' for pretty JSON formatting"
echo "      Install with: apt-get install jq (Debian/Ubuntu)"
echo "                    brew install jq (macOS)"
