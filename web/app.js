const client = new ChatClient(`ws://${window.location.host}/api`);
const chatHistory = document.getElementById('chat-history');
const chatForm = document.getElementById('chat-form');
const messageInput = document.getElementById('message-input');
const sendBtn = document.getElementById('send-btn');
const cancelBtn = document.getElementById('cancel-btn');
const providerSelect = document.getElementById('provider-select');
const modelSelect = document.getElementById('model-select');
const workerSelect = document.getElementById('worker-select');

let currentStreamHandles = {};
let isAgentThinking = false;
let initialModel = null;

let registeredCommands = [];
const suggestionsPanel = document.getElementById('suggestions-panel');

function scrollToBottom() {
    chatHistory.scrollTop = chatHistory.scrollHeight;
}

function createBubble(id, role, content, isEphemeral = false) {
    const bubble = document.createElement('div');
    bubble.className = `bubble ${role}`;
    bubble.id = id;
    if (isEphemeral) bubble.classList.add('ephemeral');

    const roleIcon = role === 'user' ? 'fa-user' : (role === 'tool' ? 'fa-wrench' : 'fa-robot');
    const roleName = role === 'user' ? 'You' : (role === 'tool' ? 'Tool' : 'Agent');

    if (role === 'tool') {
        bubble.innerHTML = `<div class="content-area"></div>`;
    } else {
        bubble.innerHTML = `
            <div class="role-label"><i class="fa-solid ${roleIcon}"></i> ${roleName}</div>
            <div class="markdown-body content-area"></div>
        `;
    }
    
    updateBubbleContent(bubble, content, role);
    chatHistory.appendChild(bubble);
    scrollToBottom();
    return bubble;
}

function updateBubbleContent(bubble, content, role) {
    const contentArea = bubble.querySelector('.content-area');
    if (role === 'tool') {
        // Just raw string for running state
        if (typeof content === 'string') {
            contentArea.innerHTML = `
                <div class="tool-card">
                    <div class="tool-header">
                        <div class="tool-title"><i class="fa-solid fa-cog fa-spin"></i> ${escapeHtml(content.split('\\n')[0])}</div>
                    </div>
                </div>
            `;
        } else {
            // It's the finished object
            const { cmd, success, output } = content;
            const shortCmd = cmd.length > 50 ? cmd.substring(0, 47) + '...' : cmd;
            const icon = success ? '<i class="fa-solid fa-check" style="color:var(--primary)"></i>' : '<i class="fa-solid fa-times" style="color:var(--accent)"></i>';

            // Surface the exit code (if the worker reported one) as a badge.
            const exitMatch = (output || '').match(/exit=(-?\d+)/);
            const exitBadge = exitMatch
                ? `<span class="tool-exit ${exitMatch[1] === '0' ? 'ok' : 'err'}">exit ${exitMatch[1]}</span>`
                : '';

            contentArea.innerHTML = `
                <div class="tool-card">
                    <div class="tool-header" onclick="this.parentElement.classList.toggle('expanded')">
                        <div class="tool-title">${icon} ${escapeHtml(shortCmd)}</div>
                        <div class="tool-status">${exitBadge}<i class="fa-solid fa-chevron-down tool-chevron"></i></div>
                    </div>
                    <div class="tool-body">
                        <div class="tool-cmd">
                            <div class="tool-cmd-label">Command</div>
                            <pre>${escapeHtml(cmd)}</pre>
                        </div>
                        <div class="tool-out">
                            <div class="tool-out-label">Output</div>
                            <pre>${escapeHtml(output)}</pre>
                        </div>
                    </div>
                </div>
            `;
        }
    } else {
        contentArea.innerHTML = marked.parse(content || '');
    }
}

function escapeHtml(unsafe) {
    return (unsafe || '')
         .toString()
         .replace(/&/g, "&amp;")
         .replace(/</g, "&lt;")
         .replace(/>/g, "&gt;")
         .replace(/"/g, "&quot;")
         .replace(/'/g, "&#039;");
}

client.onMessage = (msg) => {
    switch (msg.type) {
        case 'init':
            chatHistory.innerHTML = '';
            msg.history.forEach(m => {
                createBubble(`msg-${Math.random()}`, m.role, m.content);
            });
            
            if (msg.providers && msg.providers.length > 0) {
                providerSelect.innerHTML = '';
                msg.providers.forEach(p => {
                    const opt = document.createElement('option');
                    opt.value = p.id;
                    opt.textContent = p.name;
                    providerSelect.appendChild(opt);
                });
            }
            if (msg.provider) {
                providerSelect.value = msg.provider;
            }
            if (msg.model) {
                initialModel = msg.model;
            }
            if (msg.commands) {
                registeredCommands = msg.commands;
            }
            
            // Fetch models for current provider
            modelSelect.innerHTML = '<option>Loading...</option>';
            client.send('get_models', { provider: providerSelect.value });
            break;
            
        case 'models_list':
            if (providerSelect.value === msg.provider) {
                modelSelect.innerHTML = '';
                if (msg.models && msg.models.length > 0) {
                    msg.models.forEach(m => {
                        const opt = document.createElement('option');
                        opt.value = m;
                        opt.textContent = m;
                        modelSelect.appendChild(opt);
                    });
                    
                    if (initialModel) {
                        modelSelect.value = initialModel;
                        if (!modelSelect.value) { // The model wasn't in the list
                            modelSelect.selectedIndex = 0;
                        }
                        initialModel = null;
                    }
                } else {
                    modelSelect.innerHTML = '<option value="">No models</option>';
                }
            }
            break;
            
        case 'assistant':
            createBubble(`msg-${Math.random()}`, 'assistant', msg.text, msg.ephemeral);
            setThinking(false);
            break;

        case 'stream_begin':
            currentStreamHandles[msg.handle] = {
                bubbleId: `stream-${msg.handle}`,
                content: ''
            };
            createBubble(currentStreamHandles[msg.handle].bubbleId, 'assistant', '');
            setThinking(true);
            break;

        case 'stream_push':
            if (currentStreamHandles[msg.handle]) {
                const stream = currentStreamHandles[msg.handle];
                stream.content += msg.text;
                const bubble = document.getElementById(stream.bubbleId);
                if (bubble) {
                    updateBubbleContent(bubble, stream.content, 'assistant');
                    scrollToBottom();
                }
            }
            break;

        case 'stream_end':
            if (currentStreamHandles[msg.handle]) {
                const bubble = document.getElementById(currentStreamHandles[msg.handle].bubbleId);
                if (bubble && msg.text !== null) {
                    updateBubbleContent(bubble, msg.text, 'assistant');
                } else if (bubble && msg.text === null) {
                    bubble.remove();
                }
                delete currentStreamHandles[msg.handle];
            }
            break;

        case 'tool_begin':
            currentStreamHandles[`tool-${msg.handle}`] = {
                bubbleId: `tool-${msg.handle}`,
                content: msg.content
            };
            createBubble(currentStreamHandles[`tool-${msg.handle}`].bubbleId, 'tool', msg.content);
            scrollToBottom();
            break;

        case 'tool_update':
            // we ignore tool_update now as tool_finish will give us the rich card
            break;
            
        case 'tool_finish':
            if (currentStreamHandles[`tool-${msg.handle}`]) {
                const bubble = document.getElementById(currentStreamHandles[`tool-${msg.handle}`].bubbleId);
                if (bubble) {
                    updateBubbleContent(bubble, {
                        cmd: msg.cmd,
                        success: msg.success,
                        output: msg.output
                    }, 'tool');
                    scrollToBottom();
                }
            }
            break;

        case 'set_thinking':
            setThinking(msg.thinking);
            break;

        case 'turn_end':
            setThinking(false);
            break;

        case 'cleared':
            chatHistory.innerHTML = '';
            currentStreamHandles = {};
            setThinking(false);
            break;

        case 'config_update':
            if (msg.provider) providerSelect.value = msg.provider;
            if (msg.model) {
                if (![...modelSelect.options].some(o => o.value === msg.model)) {
                    const opt = document.createElement('option');
                    opt.value = msg.model;
                    opt.textContent = msg.model;
                    modelSelect.appendChild(opt);
                }
                modelSelect.value = msg.model;
            }
            break;
    }
};

providerSelect.addEventListener('change', () => {
    client.send('set_provider', { provider: providerSelect.value });
    modelSelect.innerHTML = '<option>Loading...</option>';
    client.send('get_models', { provider: providerSelect.value });
});

modelSelect.addEventListener('change', () => {
    if (modelSelect.value && modelSelect.value !== "Loading...") {
        client.send('set_model', { model: modelSelect.value });
    }
});

let thinkingIndicator = null;
function setThinking(isThinking) {
    if (isThinking && !thinkingIndicator) {
        thinkingIndicator = document.createElement('div');
        thinkingIndicator.className = 'thinking-indicator';
        thinkingIndicator.innerHTML = `
            <div class="dot"></div>
            <div class="dot"></div>
            <div class="dot"></div>
        `;
        chatHistory.appendChild(thinkingIndicator);
        scrollToBottom();
        sendBtn.classList.add('hidden');
        cancelBtn.classList.remove('hidden');
        isAgentThinking = true;
    } else if (!isThinking && thinkingIndicator) {
        thinkingIndicator.remove();
        thinkingIndicator = null;
        sendBtn.classList.remove('hidden');
        cancelBtn.classList.add('hidden');
        isAgentThinking = false;
    }
}

chatForm.addEventListener('submit', (e) => {
    e.preventDefault();
    const text = messageInput.value.trim();
    if (!text || isAgentThinking) return;
    
    suggestionsPanel.style.display = 'none';

    if (text.startsWith('/')) {
        createBubble(`msg-${Math.random()}`, 'user', text, true);
        client.send('command', { command: text });
    } else {
        createBubble(`msg-${Math.random()}`, 'user', text);
        client.send('send', { message: text });
        setThinking(true);
    }
    
    messageInput.value = '';
    messageInput.style.height = 'auto';
});

cancelBtn.addEventListener('click', () => {
    client.send('cancel', {});
    setThinking(false);
});

let suggestionMatches = [];
let activeSuggestion = -1;

function suggestionsVisible() {
    return suggestionsPanel.style.display === 'block' && suggestionMatches.length > 0;
}

function hideSuggestions() {
    suggestionsPanel.style.display = 'none';
    suggestionMatches = [];
    activeSuggestion = -1;
}

function renderSuggestions() {
    suggestionsPanel.innerHTML = '';
    suggestionMatches.forEach((c, i) => {
        const div = document.createElement('div');
        div.className = 'suggestion-item' + (i === activeSuggestion ? ' active' : '');
        div.innerHTML = `<span class="suggestion-name">/${c.name}</span><span class="suggestion-desc">${escapeHtml(c.description)}</span>`;
        // mousedown (not click) so it fires before the textarea loses focus.
        div.onmousedown = (e) => { e.preventDefault(); acceptSuggestion(i); };
        suggestionsPanel.appendChild(div);
    });
}

function acceptSuggestion(i) {
    const c = suggestionMatches[i];
    if (!c) return;
    messageInput.value = `/${c.name} `;
    messageInput.focus();
    hideSuggestions();
}

function updateSuggestions() {
    const text = messageInput.value;
    if (!text.startsWith('/')) {
        hideSuggestions();
        return;
    }
    const query = text.substring(1).toLowerCase();
    suggestionMatches = registeredCommands.filter(c =>
        c.name.toLowerCase().startsWith(query) ||
        c.aliases.some(a => a.toLowerCase().startsWith(query)));

    if (suggestionMatches.length === 0) {
        hideSuggestions();
        return;
    }
    activeSuggestion = 0;
    renderSuggestions();
    suggestionsPanel.style.display = 'block';
}

messageInput.addEventListener('input', function() {
    this.style.height = 'auto';
    this.style.height = Math.min(this.scrollHeight, 150) + 'px';
    updateSuggestions();
});

messageInput.addEventListener('keydown', (e) => {
    // Navigate the command palette: ↑/↓ to move, Tab to complete, Esc to close.
    if (suggestionsVisible()) {
        if (e.key === 'ArrowDown') {
            e.preventDefault();
            activeSuggestion = (activeSuggestion + 1) % suggestionMatches.length;
            renderSuggestions();
            return;
        }
        if (e.key === 'ArrowUp') {
            e.preventDefault();
            activeSuggestion = (activeSuggestion - 1 + suggestionMatches.length) % suggestionMatches.length;
            renderSuggestions();
            return;
        }
        if (e.key === 'Tab') {
            e.preventDefault();
            acceptSuggestion(activeSuggestion);
            return;
        }
        if (e.key === 'Escape') {
            e.preventDefault();
            hideSuggestions();
            return;
        }
    }
    if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        chatForm.dispatchEvent(new Event('submit'));
    }
});

if (typeof marked !== 'undefined') {
    client.connect();
} else {
    setTimeout(() => client.connect(), 500);
}
