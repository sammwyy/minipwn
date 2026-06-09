class ChatClient {
    constructor(url) {
        this.url = url;
        this.ws = null;
        this.onMessage = null;
        this.reconnectAttempts = 0;
    }

    connect() {
        this.ws = new WebSocket(this.url);
        
        this.ws.onopen = () => {
            console.log('Connected to agent');
            this.reconnectAttempts = 0;
        };

        this.ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            if (this.onMessage) {
                this.onMessage(data);
            }
        };

        this.ws.onclose = () => {
            console.log('Disconnected. Reconnecting...');
            setTimeout(() => this.connect(), Math.min(1000 * Math.pow(2, this.reconnectAttempts++), 10000));
        };
    }

    send(action, payload) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({ action, ...payload }));
        }
    }
}
