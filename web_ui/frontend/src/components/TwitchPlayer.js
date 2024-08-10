import React, { useEffect, useRef } from 'react';

const TwitchPlayer = ({ channel }) => {
    const playerRef = useRef(null);

    useEffect(() => {
        const script = document.createElement('script');
        script.src = "https://player.twitch.tv/js/embed/v1.js";
        script.async = true;
        document.body.appendChild(script);

        script.onload = () => {
            new window.Twitch.Player(playerRef.current, {
                channel: channel,
                width: '100%',
                height: '100%',
                parent: [window.location.hostname]
            });
        };

        return () => {
            document.body.removeChild(script);
        };
    }, [channel]);

    return <div ref={playerRef} className="w-full h-full"></div>;
};

export default TwitchPlayer;