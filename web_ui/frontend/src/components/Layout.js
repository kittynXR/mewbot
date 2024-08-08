import React from 'react';
import { Home, Settings, Layout as LayoutIcon } from 'lucide-react';

const Sidebar = ({ activeItem, setActiveItem }) => {
    const menuItems = [
        { name: 'Dashboard', icon: Home },
        { name: 'OBS Overlay', icon: LayoutIcon },
        { name: 'Settings', icon: Settings },
    ];

    return (
        <div className="bg-gray-800 text-white w-64 space-y-6 py-7 px-2 absolute inset-y-0 left-0 transform -translate-x-full md:relative md:translate-x-0 transition duration-200 ease-in-out">
            <nav>
                {menuItems.map((item) => (
                    <button
                        key={item.name}
                        className={`block w-full text-left py-2.5 px-4 rounded transition duration-200 ${
                            activeItem === item.name ? 'bg-gray-700' : 'hover:bg-gray-700'
                        }`}
                        onClick={() => setActiveItem(item.name)}
                    >
                        <item.icon className="inline-block mr-2 h-5 w-5" />
                        {item.name}
                    </button>
                ))}
            </nav>
        </div>
    );
};

const Layout = ({ children, activeView, setActiveView }) => {
    return (
        <div className="flex h-screen bg-gray-900 text-white">
            <Sidebar activeItem={activeView} setActiveItem={setActiveView} />
            <div className="flex-1 p-10 overflow-y-auto">
                <header className="bg-gray-800 shadow-md p-4 mb-6">
                    <h1 className="text-2xl font-bold text-white">MewBot Web UI</h1>
                </header>
                {children}
            </div>
        </div>
    );
};

export default Layout;