import React from 'react';
import { Share } from 'lucide-react';

const VRChatWorldStatus = ({ currentVRCWorld, vrchatStatus, handleShareWorld }) => {
    console.log('VRChatWorldStatus props:', { currentVRCWorld, vrchatStatus });

    return (
        <div className="bg-gray-800 p-6 rounded-lg shadow-md">
            <h2 className="text-2xl font-bold mb-4 text-white">Current VRChat World</h2>
            <p className="text-gray-300 mb-2">
                VRChat: <span className={`font-bold ${vrchatStatus ? 'text-green-500' : 'text-red-500'}`}>
                    {vrchatStatus ? 'Connected' : 'Disconnected'}
                </span>
            </p>
            {currentVRCWorld ? (
                <>
                    <p className="text-gray-300 mb-2">Name: {currentVRCWorld.name}</p>
                    <p className="text-gray-300 mb-2">Description: {currentVRCWorld.description}</p>
                    <p className="text-gray-300 mb-2">Author: {currentVRCWorld.authorName}</p>
                    <p className="text-gray-300 mb-2">Capacity: {currentVRCWorld.capacity}</p>
                    <p className="text-gray-300 mb-2">Published: {new Date(currentVRCWorld.created_at).toLocaleString()}</p>
                    <p className="text-gray-300 mb-4">Updated: {new Date(currentVRCWorld.updated_at).toLocaleString()}</p>
                    <a
                        href={`https://vrchat.com/home/world/${currentVRCWorld.id}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-blue-400 hover:text-blue-300 mb-4 inline-block"
                    >
                        View World
                    </a>
                </>
            ) : (
                <p className="text-gray-300 mb-4">Not in a world</p>
            )}
            <button
                onClick={handleShareWorld}
                className={`bg-blue-500 hover:bg-blue-600 text-white font-bold py-2 px-4 rounded flex items-center`}
                disabled={!currentVRCWorld}
            >
                <Share className="mr-2" size={16}/>
                Share with Chat
            </button>
        </div>
    );
};

export default VRChatWorldStatus;