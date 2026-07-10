import { Navigate, Route, Routes } from 'react-router-dom';
import { RequireAuth } from './auth/RequireAuth';
import { Layout } from './components/Layout';
import { LoginPage } from './pages/LoginPage';
import { RegisterPage } from './pages/RegisterPage';
import { ReceiptsPage } from './pages/ReceiptsPage';
import { UploadPage } from './pages/UploadPage';
import { ReceiptDetailPage } from './pages/ReceiptDetailPage';
import { ItemsPage } from './pages/ItemsPage';
import { AnalyticsPage } from './pages/AnalyticsPage';

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/register" element={<RegisterPage />} />
      <Route
        element={
          <RequireAuth>
            <Layout />
          </RequireAuth>
        }
      >
        <Route path="/" element={<ReceiptsPage />} />
        <Route path="/upload" element={<UploadPage />} />
        <Route path="/receipts/:id" element={<ReceiptDetailPage />} />
        <Route path="/items" element={<ItemsPage />} />
        <Route path="/analytics" element={<AnalyticsPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
