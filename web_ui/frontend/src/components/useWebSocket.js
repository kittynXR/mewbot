import { useEffect, useRef, useCallback, useState } from 'react';

function useWebSocket(url, onMessage, onError) {
    const [isConnected, setIsConnected] = useState(false);
    const socketRef = useRef(null);
    const reconnectTimeoutRef = useRef(null);
    const reconnectAttempts = useRef(0);
    const maxReconnectDelay = 30000; // 30 seconds
    const maxReconnectAttempts = 10;

    const connect = useCallback(() => {
        if (socketRef.current?.readyState === WebSocket.OPEN) return;

        if (reconnectAttempts.current >= maxReconnectAttempts) {
            console.log('Max reconnect attempts reached. Stopping reconnection.');
            onError('Max reconnect attempts reached');
            return;
        }

        console.log(`Attempting to connect WebSocket (Attempt ${reconnectAttempts.current + 1})`);

        socketRef.current = new WebSocket(url);

        socketRef.current.onopen = () => {
            console.log('WebSocket connected');
            setIsConnected(true);
            reconnectAttempts.current = 0;
        };

        socketRef.current.onmessage = (event) => {
            if (event.data === 'READY') {
                console.log('Received READY message, sending ACK');
                socketRef.current.send('ACK');
                return;
            }

            try {
                const data = JSON.parse(event.data);
                onMessage(data);
            } catch (error) {
                console.error('Failed to parse WebSocket message:', error, 'Raw message:', event.data);
            }
        };

        socketRef.current.onerror = (error) => {
            console.error('WebSocket error:', error);
            onError('WebSocket error occurred');
        };

        socketRef.current.onclose = (event) => {
            console.log('WebSocket disconnected:', event.code, event.reason);
            setIsConnected(false);
            reconnectAttempts.current++;
            const delay = Math.min(maxReconnectDelay, Math.pow(2, reconnectAttempts.current) * 1000);
            console.log(`Attempting to reconnect in ${delay}ms`);
            reconnectTimeoutRef.current = setTimeout(connect, delay);
        };
    }, [url, onMessage, onError, maxReconnectAttempts]);

    useEffect(() => {
        connect();
        return () => {
            if (socketRef.current) {
                socketRef.current.close();
            }
            if (reconnectTimeoutRef.current) {
                clearTimeout(reconnectTimeoutRef.current);
            }
        };
    }, [connect]);

    const sendMessage = useCallback((message) => {
        if (socketRef.current?.readyState === WebSocket.OPEN) {
            socketRef.current.send(JSON.stringify(message));
        } else {
            console.error('WebSocket is not connected');
            onError('WebSocket is not connected');
        }
    }, [onError]);

    return { sendMessage, isConnected };
}

export default useWebSocket;