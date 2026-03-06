<script lang="ts">
    import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
    import { listen } from "@tauri-apps/api/event";
    import "$lib/styles/chat.css";
</script>

<div class="app-container">
    <aside class="sidebar-container">
        <nav class="spaces-bar">
            <div class="dm-and-divider">
                <img src="" />

            </div>
        </nav>

        <section class="space-info-and-rooms">

        </section>

        <footer class="user-bar">

        </footer>

    </aside>

    <main class ="app-layout">

    </main>
</div>

