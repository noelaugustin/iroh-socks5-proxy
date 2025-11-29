#!/usr/bin/env python3
"""
Integration test framework for Iroh Tunnel persistence behavior.

Tests:
1. Server has stable Node ID across restarts
2. Client has random Node ID each run (ephemeral)
3. Server never persists peer connections
4. Client persists server peer ID
5. Multiple clients can connect to same server
"""

import subprocess
import time
import os
import re
import signal
import shutil
from pathlib import Path
from typing import Optional, Tuple
from dataclasses import dataclass


@dataclass
class ProcessInfo:
    """Information about a running tunnel process."""
    process: subprocess.Popen
    node_id: Optional[str]
    port: int
    work_dir: Path
    mode: str  # "server" or "client"


class Colors:
    """ANSI color codes for terminal output."""
    BLUE = '\033[0;34m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    RED = '\033[0;31m'
    NC = '\033[0m'  # No Color
    BOLD = '\033[1m'


class TunnelTester:
    """Framework for testing tunnel persistence behavior."""
    
    def __init__(self, binary_path: str = "./target/debug/tunnel"):
        # Resolve binary path relative to script directory
        script_dir = Path(__file__).parent
        if not Path(binary_path).is_absolute():
            binary_path = str(script_dir / binary_path)
        
        self.binary_path = binary_path
        self.test_dirs = []
        self.processes = []
        
        # Verify binary exists
        if not Path(self.binary_path).exists():
            raise FileNotFoundError(
                f"Binary not found at: {self.binary_path}\n"
                f"Please run 'cargo build' first."
            )
        
    def cleanup(self):
        """Clean up all test processes and directories."""
        print(f"{Colors.YELLOW}Cleaning up...{Colors.NC}")
        
        # Kill all processes
        for proc_info in self.processes:
            try:
                proc_info.process.terminate()
                proc_info.process.wait(timeout=2)
            except:
                try:
                    proc_info.process.kill()
                except:
                    pass
        
        # Remove test directories
        for test_dir in self.test_dirs:
            if test_dir.exists():
                shutil.rmtree(test_dir)
        
        self.processes = []
        self.test_dirs = []
        print(f"{Colors.GREEN}Cleanup complete{Colors.NC}\n")
    
    def build_binary(self):
        """Build the binary before testing."""
        print(f"{Colors.BLUE}Building the binary...{Colors.NC}")
        result = subprocess.run(["cargo", "build"], capture_output=True, text=True)
        if result.returncode != 0:
            print(f"{Colors.RED}Build failed:{Colors.NC}\n{result.stderr}")
            return False
        print(f"{Colors.GREEN}Build complete{Colors.NC}\n")
        return True
    
    def start_server(self, name: str, port: int = 8080) -> Optional[ProcessInfo]:
        """Start a server instance."""
        work_dir = Path(f".test_{name.lower().replace(' ', '_')}")
        work_dir.mkdir(exist_ok=True)
        self.test_dirs.append(work_dir)
        
        print(f"{Colors.BLUE}Starting {name} (port {port})...{Colors.NC}")
        
        # Start process
        process = subprocess.Popen(
            [self.binary_path, "-p", str(port)],
            cwd=work_dir,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1
        )
        
        # Wait for startup and extract Node ID
        node_id = None
        start_time = time.time()
        while time.time() - start_time < 5:
            line = process.stdout.readline()
            if not line:
                break
            
            # Look for Node ID
            match = re.search(r'Node ID: ([a-f0-9]+)', line)
            if match:
                node_id = match.group(1)
                break
        
        if not node_id:
            print(f"{Colors.RED}Failed to start {name} (no Node ID found){Colors.NC}")
            process.terminate()
            return None
        
        proc_info = ProcessInfo(
            process=process,
            node_id=node_id,
            port=port,
            work_dir=work_dir,
            mode="server"
        )
        self.processes.append(proc_info)
        
        print(f"{Colors.GREEN}{name} started{Colors.NC}")
        print(f"  Node ID: {node_id}")
        print(f"  Port: {port}")
        print(f"  Directory: {work_dir}\n")
        
        return proc_info
    
    def start_client(self, name: str, server_node_id: str, port: int = 9080) -> Optional[ProcessInfo]:
        """Start a client instance."""
        work_dir = Path(f".test_{name.lower().replace(' ', '_')}")
        work_dir.mkdir(exist_ok=True)
        self.test_dirs.append(work_dir)
        
        print(f"{Colors.BLUE}Starting {name} (connecting to {server_node_id[:16]}...)...{Colors.NC}")
        
        # Start process
        process = subprocess.Popen(
            [self.binary_path, "-p", str(port), "-c", server_node_id],
            cwd=work_dir,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1
        )
        
        # Wait for startup and extract Node ID
        node_id = None
        connected = False
        start_time = time.time()
        
        while time.time() - start_time < 10:
            line = process.stdout.readline()
            if not line:
                break
            
            # Look for Node ID
            match = re.search(r'Node ID: ([a-f0-9]+)', line)
            if match:
                node_id = match.group(1)
            
            # Check for connection
            if "Connected to peer" in line or "✅ Connected" in line:
                connected = True
                break
        
        if not node_id or not connected:
            print(f"{Colors.RED}Failed to start {name}{Colors.NC}")
            if not node_id:
                print(f"  Reason: No Node ID found")
            if not connected:
                print(f"  Reason: Did not connect to server")
            process.terminate()
            return None
        
        proc_info = ProcessInfo(
            process=process,
            node_id=node_id,
            port=port,
            work_dir=work_dir,
            mode="client"
        )
        self.processes.append(proc_info)
        
        print(f"{Colors.GREEN}{name} started and connected{Colors.NC}")
        print(f"  Node ID: {node_id}")
        print(f"  Port: {port}\n")
        
        return proc_info
    
    def stop_process(self, proc_info: ProcessInfo):
        """Stop a specific process."""
        print(f"{Colors.YELLOW}Stopping process (Node ID: {proc_info.node_id[:16]}...)...{Colors.NC}")
        try:
            proc_info.process.terminate()
            proc_info.process.wait(timeout=2)
        except:
            proc_info.process.kill()
        
        if proc_info in self.processes:
            self.processes.remove(proc_info)
        print(f"{Colors.GREEN}Process stopped{Colors.NC}\n")
    
    def check_file_exists(self, proc_info: ProcessInfo, filename: str) -> bool:
        """Check if a file exists in the process working directory."""
        return (proc_info.work_dir / filename).exists()
    
    def print_test_header(self, scenario_num: int, title: str):
        """Print a test scenario header."""
        print(f"\n{Colors.BLUE}{'=' * 60}{Colors.NC}")
        print(f"{Colors.BLUE}{Colors.BOLD}TEST SCENARIO {scenario_num}: {title}{Colors.NC}")
        print(f"{Colors.BLUE}{'=' * 60}{Colors.NC}\n")
    
    def print_result(self, passed: bool, message: str):
        """Print a test result."""
        if passed:
            print(f"{Colors.GREEN}✓ {message}{Colors.NC}")
        else:
            print(f"{Colors.RED}✗ {message}{Colors.NC}")
        return passed
    
    def test_scenario_1_stable_server_id(self) -> bool:
        """Test that server has stable Node ID across restarts."""
        self.print_test_header(1, "Server Has Stable Node ID")
        
        print(f"{Colors.YELLOW}Step 1: Starting server first time{Colors.NC}")
        server = self.start_server("Server", port=8080)
        if not server:
            return self.print_result(False, "Failed to start server")
        
        first_node_id = server.node_id
        time.sleep(1)
        
        print(f"{Colors.YELLOW}Step 2: Stopping server{Colors.NC}")
        self.stop_process(server)
        time.sleep(1)
        
        print(f"{Colors.YELLOW}Step 3: Restarting server{Colors.NC}")
        server2 = self.start_server("Server", port=8080)
        if not server2:
            return self.print_result(False, "Failed to restart server")
        
        second_node_id = server2.node_id
        
        # Verify
        result = first_node_id == second_node_id
        self.print_result(result, f"Server has stable Node ID: {first_node_id == second_node_id}")
        if result:
            print(f"  First run:  {first_node_id}")
            print(f"  Second run: {second_node_id}")
        
        self.stop_process(server2)
        return result
    
    def test_scenario_2_ephemeral_client_id(self) -> bool:
        """Test that client has random Node ID each run."""
        self.print_test_header(2, "Client Has Ephemeral Node ID")
        
        print(f"{Colors.YELLOW}Step 1: Starting server{Colors.NC}")
        server = self.start_server("Server", port=8081)
        if not server:
            return self.print_result(False, "Failed to start server")
        
        time.sleep(1)
        
        print(f"{Colors.YELLOW}Step 2: Starting client first time{Colors.NC}")
        client1 = self.start_client("Client1", server.node_id, port=9081)
        if not client1:
            self.stop_process(server)
            return self.print_result(False, "Failed to start client")
        
        first_node_id = client1.node_id
        time.sleep(1)
        
        print(f"{Colors.YELLOW}Step 3: Stopping and restarting client{Colors.NC}")
        self.stop_process(client1)
        time.sleep(1)
        
        client2 = self.start_client("Client2", server.node_id, port=9082)
        if not client2:
            self.stop_process(server)
            return self.print_result(False, "Failed to restart client")
        
        second_node_id = client2.node_id
        
        # Verify
        result = first_node_id != second_node_id
        self.print_result(result, f"Client has different Node ID each run: {first_node_id != second_node_id}")
        if result:
            print(f"  First run:  {first_node_id}")
            print(f"  Second run: {second_node_id}")
        
        self.stop_process(client2)
        self.stop_process(server)
        return result
    
    def test_scenario_3_server_no_peer_persistence(self) -> bool:
        """Test that server never creates .tunnel_peer file."""
        self.print_test_header(3, "Server Never Persists Peer Connections")
        
        print(f"{Colors.YELLOW}Step 1: Starting server{Colors.NC}")
        server = self.start_server("Server", port=8082)
        if not server:
            return self.print_result(False, "Failed to start server")
        
        time.sleep(1)
        
        print(f"{Colors.YELLOW}Step 2: Connecting client to server{Colors.NC}")
        client = self.start_client("Client", server.node_id, port=9083)
        if not client:
            self.stop_process(server)
            return self.print_result(False, "Failed to start client")
        
        time.sleep(2)
        
        # Verify server has no .tunnel_peer
        server_has_peer_file = self.check_file_exists(server, ".tunnel_peer")
        result = not server_has_peer_file
        
        self.print_result(result, f"Server does NOT have .tunnel_peer file: {result}")
        
        self.stop_process(client)
        self.stop_process(server)
        return result
    
    def test_scenario_4_client_peer_persistence(self) -> bool:
        """Test that client persists server peer ID."""
        self.print_test_header(4, "Client Persists Server Peer ID")
        
        print(f"{Colors.YELLOW}Step 1: Starting server{Colors.NC}")
        server = self.start_server("Server", port=8083)
        if not server:
            return self.print_result(False, "Failed to start server")
        
        time.sleep(1)
        
        print(f"{Colors.YELLOW}Step 2: Connecting client to server{Colors.NC}")
        client = self.start_client("Client", server.node_id, port=9084)
        if not client:
            self.stop_process(server)
            return self.print_result(False, "Failed to start client")
        
        time.sleep(2)
        
        # Verify client has .tunnel_peer
        client_has_peer_file = self.check_file_exists(client, ".tunnel_peer")
        result = client_has_peer_file
        
        self.print_result(result, f"Client has .tunnel_peer file: {result}")
        
        self.stop_process(client)
        self.stop_process(server)
        return result
    
    def test_scenario_5_multiple_clients(self) -> bool:
        """Test that server accepts multiple different clients."""
        self.print_test_header(5, "Server Accepts Multiple Clients")
        
        print(f"{Colors.YELLOW}Step 1: Starting server{Colors.NC}")
        server = self.start_server("Server", port=8084)
        if not server:
            return self.print_result(False, "Failed to start server")
        
        time.sleep(1)
        
        print(f"{Colors.YELLOW}Step 2: Connecting Client 1{Colors.NC}")
        client1 = self.start_client("Client1", server.node_id, port=9085)
        if not client1:
            self.stop_process(server)
            return self.print_result(False, "Failed to start Client 1")
        
        time.sleep(1)
        
        print(f"{Colors.YELLOW}Step 3: Disconnecting Client 1{Colors.NC}")
        self.stop_process(client1)
        time.sleep(1)
        
        print(f"{Colors.YELLOW}Step 4: Connecting Client 2{Colors.NC}")
        client2 = self.start_client("Client2", server.node_id, port=9086)
        if not client2:
            self.stop_process(server)
            return self.print_result(False, "Failed to start Client 2")
        
        result = True
        self.print_result(result, "Multiple clients can connect sequentially")
        
        self.stop_process(client2)
        self.stop_process(server)
        return result
    
    def run_all_tests(self):
        """Run all test scenarios."""
        print(f"\n{Colors.BLUE}{Colors.BOLD}╔════════════════════════════════════════════════════════════╗{Colors.NC}")
        print(f"{Colors.BLUE}{Colors.BOLD}║     Iroh Tunnel Persistence - Integration Test Suite      ║{Colors.NC}")
        print(f"{Colors.BLUE}{Colors.BOLD}╚════════════════════════════════════════════════════════════╝{Colors.NC}\n")
        
        if not self.build_binary():
            print(f"{Colors.RED}Build failed, aborting tests{Colors.NC}")
            return
        
        results = []
        
        try:
            results.append(("Stable Server ID", self.test_scenario_1_stable_server_id()))
            self.cleanup()
            time.sleep(1)
            
            results.append(("Ephemeral Client ID", self.test_scenario_2_ephemeral_client_id()))
            self.cleanup()
            time.sleep(1)
            
            results.append(("Server No Peer Persistence", self.test_scenario_3_server_no_peer_persistence()))
            self.cleanup()
            time.sleep(1)
            
            results.append(("Client Peer Persistence", self.test_scenario_4_client_peer_persistence()))
            self.cleanup()
            time.sleep(1)
            
            results.append(("Multiple Clients", self.test_scenario_5_multiple_clients()))
            self.cleanup()
            
        except KeyboardInterrupt:
            print(f"\n{Colors.YELLOW}Tests interrupted by user{Colors.NC}")
            self.cleanup()
            return
        except Exception as e:
            print(f"\n{Colors.RED}Test error: {e}{Colors.NC}")
            self.cleanup()
            return
        
        # Print summary
        print(f"\n{Colors.BLUE}{Colors.BOLD}{'=' * 60}{Colors.NC}")
        print(f"{Colors.BLUE}{Colors.BOLD}TEST SUMMARY{Colors.NC}")
        print(f"{Colors.BLUE}{Colors.BOLD}{'=' * 60}{Colors.NC}\n")
        
        passed = sum(1 for _, result in results if result)
        total = len(results)
        
        for name, result in results:
            status = f"{Colors.GREEN}PASS{Colors.NC}" if result else f"{Colors.RED}FAIL{Colors.NC}"
            print(f"  {status}  {name}")
        
        print()
        if passed == total:
            print(f"{Colors.GREEN}{Colors.BOLD}✓ All {total} tests passed!{Colors.NC}")
        else:
            print(f"{Colors.YELLOW}{passed}/{total} tests passed{Colors.NC}")
        print()


def main():
    tester = TunnelTester()
    try:
        tester.run_all_tests()
    finally:
        tester.cleanup()


if __name__ == "__main__":
    main()
