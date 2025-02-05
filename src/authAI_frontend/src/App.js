import React, { useEffect, useState } from 'react';
import Masonry from 'react-masonry-css';
import { authAI_backend } from './declarations/authAI_backend';
import './App.scss';

const App = () => {
  const [uploads, setUploads] = useState([]);
  const [progress, setProgress] = useState(null);
  const [error, setError] = useState(null);

  // Fetch stored images from the backend
  useEffect(() => {
    fetchUploads();
  }, []);

  const fetchUploads = async () => {
    try {
      const imageNames = await authAI_backend.list_images();
      const imageDetails = await Promise.all(
        imageNames.map(async (name) => {
          const storedImage = await authAI_backend.get_image(name);
          const blob = new Blob([new Uint8Array(storedImage.content)], { type: 'image/jpeg' });
          const url = URL.createObjectURL(blob);
          return { name, url };
        })
      );
      setUploads(imageDetails);
    } catch (e) {
      console.error('Failed to fetch images:', e);
      setError('Could not fetch images.');
    }
  };

  const uploadPhotos = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'image/*';
    input.multiple = true;
    input.onchange = async () => {
      setProgress(0);
      try {
        const files = Array.from(input.files);
        for (let file of files) {
          const reader = new FileReader();
          reader.readAsArrayBuffer(file);

          const content = await new Promise((resolve) => {
            reader.onload = () => resolve(new Uint8Array(reader.result));
          });

          await authAI_backend.store_image(file.name, content);
        }
        fetchUploads(); // Refresh the list of images
      } catch (e) {
        console.error('Error uploading image:', e);
        setError('Could not upload images.');
      } finally {
        setProgress(null);
      }
    };
    input.click();
  };

  const deletePhoto = async (name) => {
    try {
      await authAI_backend.delete_image(name);
      fetchUploads(); // Refresh the list of images
    } catch (e) {
      console.error('Error deleting image:', e);
      setError('Could not delete image.');
    }
  };

  return (
      <div>

        <button >Click Me</button>
      </div>    
  );
};

export default App;
