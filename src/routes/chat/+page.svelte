<script lang="ts">
    import "$lib/styles/chat.css";

    let sidebarCollapsed = $state(false);
    let collapsed: Record<string, boolean> = $state({});
    function toggleCategory(name: string) { collapsed[name] = !collapsed[name]; }

    let muted = $state(false);
    let deafened = $state(false);

    const spaceName = "echelon";
    const homeserver = "matrix.org";
    const username = "Clumsy ☆";

    const categories = [
        { name: "rooms", rooms: [
            { name: "announcements",       type: "text",  encrypted: true  },
            { name: "general",             type: "text",  encrypted: true  },
            { name: "very trustworthy",    type: "text",  encrypted: false },
            { name: "very trustworthy x2", type: "text",  encrypted: false },
        ]},
        { name: "cooking", rooms: [
            { name: "cookies", type: "text", encrypted: false },
            { name: "creme",   type: "text", encrypted: true  },
        ]},
        { name: "voice rooms", rooms: [
            { name: "voice room", type: "voice", encrypted: false },
        ]},
    ];

    const dmRooms = [{ name: "boo" }, { name: "boohoo" }];
</script>

<div class="app-container" class:sidebar-collapsed={sidebarCollapsed}>

    <nav class="spaces-bar">
        <img src="/dm.svg" class="spaces-dm-icon" alt="direct messages" />
        <div class="spaces-divider"></div>

        <div class="space active"><span>ec</span></div>
        <div class="space"><span>sp</span></div>

        <button
            class="spaces-collapse-btn"
            class:rotated={sidebarCollapsed}
            onclick={() => sidebarCollapsed = !sidebarCollapsed}
            title="collapse sidebar"
        >
            <img src="/dropdown.svg" class="spaces-collapse-arrow" alt="collapse" />
        </button>
    </nav>

    <aside class="sidebar-container">

        <header class="space-nameinfo">
            <div class="space-nameinfo-text">
                <span class="space-title">{spaceName}</span>
                <span class="space-homeserver">{homeserver}</span>
            </div>
            <img src="/dropdown.svg" class="space-dropdown-arrow" alt="options" />
        </header>

        <div class="space-nameinfo-divider"></div>

        <section class="rooms">

            {#each categories as category}
                <div class="sub-space" onclick={() => toggleCategory(category.name)}>
                    <img
                        src="/dropdown.svg"
                        class="sub-space-arrow"
                        class:sub-space-arrow-collapsed={collapsed[category.name]}
                        alt=""
                    />
                    <span>{category.name}</span>
                </div>
                {#if !collapsed[category.name]}
                    {#each category.rooms as room}
                        <div class="room" class:voice-room={room.type === 'voice'}>
                            <img
                                src={room.type === 'voice' ? '/voicechat.svg' : room.encrypted ? '/encrypted.svg' : '/unencrypted.svg'}
                                class="room-icon"
                                alt=""
                            />
                            <span>{room.name}</span>
                        </div>
                    {/each}
                {/if}
            {/each}

            {#each dmRooms as room}
                <div class="room dm-room">
                    <img src="/dm.svg" class="room-icon" alt="" />
                    <span>{room.name}</span>
                </div>
            {/each}

        </section>

        <footer class="user-bar">
            <div class="user-bar-pfp-wrapper">
                <div class="pfp-placeholder"></div>
                <div class="activity-status online"></div>
            </div>
            <div class="user-bar-info">
                <span class="user-bar-name">{username}</span>
                <span class="user-bar-homeserver">{homeserver}</span>
            </div>
            <div class="user-bar-controls">
                <button class="control-btn" class:active={muted} onclick={() => muted = !muted} title={muted ? "unmute" : "mute"}>
                    <img src={muted ? "/muted.svg" : "/unmuted.svg"} alt="mute" />
                </button>
                <button class="control-btn" class:active={deafened} onclick={() => deafened = !deafened} title={deafened ? "undeafen" : "deafen"}>
                    <img src={deafened ? "/deafened.svg" : "/undeafened.svg"} alt="deafen" />
                </button>
            </div>
        </footer>

    </aside>

    <main class="app-layout">
        <!-- chat area — coming tomorrow -->
    </main>

</div>
