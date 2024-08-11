import React, { useState, useEffect, useRef, useCallback } from 'react';

const Chatbox = ({ messages = [] }) => {
    const [renderedMessages, setRenderedMessages] = useState([]);
    const [emotes, setEmotes] = useState({});
    const chatboxRef = useRef(null);

    useEffect(() => {
        console.log('Messages prop in Chatbox:', messages);
        setRenderedMessages(messages);
    }, [messages]);

    // Fetch 7TV emotes
    useEffect(() => {
        const fetchEmotes = async () => {
            try {
                const response = await fetch('https://7tv.io/v3/emote-sets/global');
                if (!response.ok) {
                    throw new Error(`HTTP error! status: ${response.status}`);
                }
                const data = await response.json();
                const emoteMap = {};
                data.emotes.forEach(emote => {
                    emoteMap[emote.name] = emote.data.host.url + '/2x.webp';
                });
                setEmotes(emoteMap);
            } catch (error) {
                console.error('Error fetching 7TV emotes:', error);
                setEmotes({});
            }
        };

        fetchEmotes();
    }, []);

    const renderMessage = useCallback((message) => {
        if (typeof message !== 'string') {
            console.error('Invalid message format:', message);
            return null;
        }

        const words = message.split(' ');
        return words.map((word, index) => {
            if (emotes[word]) {
                return (
                    <img
                        key={index}
                        src={emotes[word]}
                        alt={word}
                        title={word}
                        className="inline-block align-middle"
                        style={{ height: '1.5em' }}
                    />
                );
            }
            return <span key={index}>{word} </span>;
        });
    }, [emotes]);

    useEffect(() => {
        if (chatboxRef.current) {
            chatboxRef.current.scrollTop = chatboxRef.current.scrollHeight;
        }
    }, [renderedMessages]);

    return (
        <div
            ref={chatboxRef}
            className="bg-gray-800 p-4 rounded-lg shadow-md overflow-y-auto h-full"
        >
            {renderedMessages.length === 0 ? (
                <p className="text-gray-400">No messages yet.</p>
            ) : (
                renderedMessages.map((message, index) => (
                    <div key={index} className="mb-2 text-white">
                        {renderMessage(message)}
                    </div>
                ))
            )}
        </div>
    );
};

export default Chatbox;