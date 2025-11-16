#!/usr/bin/env node
/**
 * Example Node.js client for slopdrop Web API
 * Demonstrates how to interact with slopdrop from JavaScript
 *
 * Usage:
 *   node examples/web_api_client.js
 */

class SlopdropClient {
    constructor(baseUrl = 'http://127.0.0.1:8080') {
        this.baseUrl = baseUrl;
    }

    async eval(code, isAdmin = false) {
        const response = await fetch(`${this.baseUrl}/api/eval`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ code, is_admin: isAdmin })
        });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${await response.text()}`);
        }

        return await response.json();
    }

    async more() {
        const response = await fetch(`${this.baseUrl}/api/more`);

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${await response.text()}`);
        }

        return await response.json();
    }

    async history(limit = 10) {
        const response = await fetch(`${this.baseUrl}/api/history?limit=${limit}`);

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${await response.text()}`);
        }

        return await response.json();
    }

    async rollback(commitHash) {
        const response = await fetch(`${this.baseUrl}/api/rollback`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ commit_hash: commitHash })
        });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${await response.text()}`);
        }

        return await response.json();
    }

    async health() {
        const response = await fetch(`${this.baseUrl}/api/health`);

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${await response.text()}`);
        }

        return await response.json();
    }
}

async function main() {
    const client = new SlopdropClient();

    try {
        // Check health
        console.log('Checking server health...');
        const health = await client.health();
        console.log(`✓ Server is ${health.status}\n`);

        // Simple evaluation
        console.log('Evaluating: expr {1 + 1}');
        let result = await client.eval('expr {1 + 1}');
        console.log(`Result: ${result.output.join('\n')}`);
        console.log(`Error: ${result.is_error}\n`);

        // Set a variable
        console.log('Evaluating: set myvar "Hello from JavaScript!"');
        result = await client.eval('set myvar "Hello from JavaScript!"');
        console.log(`Result: ${result.output.join('\n')}\n`);

        // Read the variable
        console.log('Evaluating: set myvar');
        result = await client.eval('set myvar');
        console.log(`Result: ${result.output.join('\n')}\n`);

        // Define a procedure
        console.log('Defining fibonacci procedure...');
        const fibCode = `
proc fibonacci {n} {
    if {$n <= 1} {
        return $n
    }
    expr {[fibonacci [expr {$n - 1}]] + [fibonacci [expr {$n - 2}]]}
}
        `.trim();
        result = await client.eval(fibCode, true);
        console.log(`Result: ${result.output.join('\n')}\n`);

        // Call the procedure
        console.log('Evaluating: fibonacci 10');
        result = await client.eval('fibonacci 10');
        console.log(`Result: ${result.output.join('\n')}\n`);

        // Get history
        console.log('Getting git history...');
        const history = await client.history(5);
        console.log('Recent commits:');
        for (const commit of history.history) {
            console.log(`  ${commit.commit_id.substring(0, 8)} - ${commit.author} - ${commit.message}`);
        }
        console.log();

        // Test pagination
        console.log('Testing pagination with large output...');
        result = await client.eval('for {set i 0} {$i < 50} {incr i} { puts "Line $i" }');
        console.log(`Got ${result.output.length} lines`);
        console.log(`More available: ${result.more_available}`);

        if (result.more_available) {
            console.log('\nGetting more output...');
            const moreResult = await client.more();
            console.log(`Got ${moreResult.output.length} more lines`);
            console.log(`Still more available: ${moreResult.more_available}`);
        }

        console.log('\n✓ All examples completed successfully!');

    } catch (error) {
        if (error.cause?.code === 'ECONNREFUSED') {
            console.error('Error: Cannot connect to slopdrop server');
            console.error('Make sure to start the server first:');
            console.error('  ./target/release/slopdrop --web');
            process.exit(1);
        }
        throw error;
    }
}

main().catch(error => {
    console.error(`Error: ${error.message}`);
    process.exit(1);
});
