import React, { useCallback } from 'react';

const Chat = ({ state, dispatch, sendMessage, isConnected }) => {
    const handleSendChat = useCallback((e) => {
        e.preventDefault();
        if (state.chatMessage.trim() === '' || !isConnected) return;

        sendMessage({
            type: 'sendChat',
            message: state.chatMessage,
            destination: state.chatDestination,
            additionalStreams: state.additionalStreams.filter((_, index) => state.additionalStreamToggles[index])
        });

        dispatch({ type: 'SET_CHAT_MESSAGE', payload: '' });
    }, [state.chatMessage, state.chatDestination, state.additionalStreams, state.additionalStreamToggles, sendMessage, isConnected, dispatch]);

    return (
        <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-2">
            <h2 className="text-2xl font-bold mb-4 text-white">Chat</h2>
            <form onSubmit={handleSendChat} className="mb-4">
                <div className="flex flex-wrap items-center mb-2">
                    {Object.entries(state.chatDestination).map(([key, value]) => (
                        <button
                            key={key}
                            type="button"
                            onClick={() => dispatch({type: 'SET_CHAT_DESTINATION', payload: {[key]: !value}})}
                            className={`mr-2 mb-2 px-4 py-2 rounded ${value ? 'bg-blue-500' : 'bg-gray-600'}`}
                        >
                            {key.charAt(0).toUpperCase() + key.slice(1)}
                        </button>
                    ))}
                    {state.additionalStreams.map((stream, index) => (
                        <button
                            key={index}
                            type="button"
                            onClick={() => dispatch({type: 'TOGGLE_ADDITIONAL_STREAM', payload: index})}
                            className={`mr-2 mb-2 px-4 py-2 rounded ${state.additionalStreamToggles[index] ? 'bg-blue-500' : 'bg-gray-600'}`}
                        >
                            {stream}
                        </button>
                    ))}
                </div>
                <div className="flex">
                    <input
                        type="text"
                        value={state.chatMessage}
                        onChange={(e) => dispatch({type: 'SET_CHAT_MESSAGE', payload: e.target.value})}
                        className="flex-grow mr-2 px-4 py-2 bg-gray-700 text-white rounded"
                        placeholder="Type your message..."
                    />
                    <button
                        type="submit"
                        className="bg-green-500 hover:bg-green-600 text-white font-bold py-2 px-4 rounded"
                    >
                        Send
                    </button>
                </div>
            </form>
        </div>
    );
};

export default Chat;