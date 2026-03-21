import React from 'react';
import ClipboardHistory from './ClipboardHistory';

interface ClipboardPanelProps {
    onClose: () => void;
}

const ClipboardPanel: React.FC<ClipboardPanelProps> = ({ onClose }) => {
    return <ClipboardHistory onClose={onClose} />;
};

export default ClipboardPanel;
