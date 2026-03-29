To transform your Axum proxy into a **Context-Compressed, Disk-Aware MCP Server**, we need to orchestrate several distinct "Jobs." Below is the logical breakdown followed by the Mermaid architecture.

### 📋 The Job List

1.  **Request Interception & State Check**: 
    * Monitor the `messages` array length. 
    * If `len > 6`, trigger the **Architect Worker**.
2.  **Architect Extraction (The "Compression" Job)**:
    * Identify the oldest $N-5$ messages.
    * Extract code blocks, rules, and technical decisions.
    * Summarize the "Intent" and "Task Progress" into a structured Manifest.
3.  **Disk-Aware Sync (The "MCP" Job)**:
    * Take the extracted code blocks and use the `write_file` MCP tool to save them to `./workspaces/{id}/`.
    * Update the File Map in the Manifest to point to these physical files.
4.  **Context Injection**:
    * Replace the dropped messages with the **State Manifest** (Markdown).
    * Inform the LLM that it has an active **MCP File System Tool** available for this workspace.
5.  **Unified Token Accounting**:
    * Calculate: $Tokens_{Architect} + Tokens_{Main\_Chat} + Tokens_{MCP\_Overhead}$.
    * Send the aggregated usage to the **ClickHouseLogger**.

---

### 🏗️ Architecture: MCP Disk-Aware Proxy

This diagram illustrates how the request flows through your Axum handler, triggers the Architect for compression, and interacts with the local filesystem via MCP.

```mermaid
graph TD
    subgraph Client_Tier [Client Layer]
        User[User/OpenWebUI]
    end

    subgraph API_Proxy [Axum Rust Server]
        Handler[Chat Handler]
        State[AppState: Bedrock Client + Logger]
        Compressor{Check Message Count > 6?}
        Architect[[Architect Worker]]
        MCPServer[MCP Server Context]
    end

    subgraph Storage_Tier [Persistence Layer]
        Disk[(Local Workspace /workspaces/id/)]
        Manifests[(Summary Files /summaries/id.md)]
        Logs[(ClickHouse Usage Logs)]
    end

    subgraph External [AI Providers]
        Bedrock[AWS Bedrock: Claude 3.5/Haiku]
    end

    %% Flow
    User -->|JSON Request| Handler
    Handler --> Compressor
    
    %% Compression Logic
    Compressor -->|Yes: n > 6| Architect
    Architect -->|Call: Summarize & Extract Code| Bedrock
    Architect -->|Update Manifest| Manifests
    Architect -->|write_file| Disk
    
    %% MCP Execution
    Architect -.->|Returns updated| MCPServer
    MCPServer -->|Inject Manifest| Handler
    
    %% Normal Flow
    Compressor -->|No: n <= 6| Handler
    Handler -->|Execute Chat| Bedrock
    
    %% Usage Logging
    Bedrock -->|Usage Data| State
    State -->|Total Tokens| Logs
```

### Key Architectural Shifts:
* **Decoupled Intelligence**: The **Architect** uses a fast model (Haiku) to manage the disk, while the **Main Handler** uses a smart model (Sonnet/Opus) for the actual conversation.
* **The Manifest as a Bootloader**: The `.md` file in `/summaries/` acts as the "system image" that re-syncs the LLM's brain every time you clear the history.
* **MCP as the Interface**: Instead of the LLM "guessing" what's in the files, it uses standard MCP `list_directory` or `read_file` tools to pull only the code it actually needs to see for the current user prompt.

**Would you like me to write the Rust code for the `Architect Worker` that handles the extraction and file-writing logic?**