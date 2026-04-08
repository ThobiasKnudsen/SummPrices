import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../providers/auth_provider.dart';
import '../providers/receipt_provider.dart';
import '../models/receipt.dart';
import 'capture_screen.dart';
import 'items_screen.dart';
import 'receipt_detail_screen.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  int _currentIndex = 0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<ReceiptProvider>().loadReceipts();
    });
  }

  Future<void> _openCapture() async {
    final receiptId = await Navigator.of(context).push<String>(
      MaterialPageRoute(builder: (_) => const CaptureScreen()),
    );

    if (receiptId != null && mounted) {
      Navigator.of(context).push(
        MaterialPageRoute(
          builder: (_) => ReceiptDetailScreen(receiptId: receiptId),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final auth = context.watch<AuthProvider>();

    return Scaffold(
      appBar: AppBar(
        title: const Text('Kvitteringsapp'),
        actions: [
          IconButton(
            icon: const Icon(Icons.logout),
            onPressed: () => auth.logout(),
          ),
        ],
      ),
      body: IndexedStack(
        index: _currentIndex,
        children: const [
          _ReceiptsTab(),
          _ItemsTab(),
          _AnalyticsTab(),
        ],
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: _openCapture,
        child: const Icon(Icons.camera_alt),
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _currentIndex,
        onDestinationSelected: (i) => setState(() => _currentIndex = i),
        destinations: const [
          NavigationDestination(icon: Icon(Icons.receipt), label: 'Receipts'),
          NavigationDestination(icon: Icon(Icons.list), label: 'Items'),
          NavigationDestination(icon: Icon(Icons.bar_chart), label: 'Analytics'),
        ],
      ),
    );
  }
}

class _ReceiptsTab extends StatelessWidget {
  const _ReceiptsTab();

  @override
  Widget build(BuildContext context) {
    final provider = context.watch<ReceiptProvider>();

    if (provider.isLoading && provider.receipts.isEmpty) {
      return const Center(child: CircularProgressIndicator());
    }

    if (provider.receipts.isEmpty) {
      return const Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.receipt_long, size: 64, color: Colors.grey),
            SizedBox(height: 16),
            Text('No receipts yet', style: TextStyle(fontSize: 18, color: Colors.grey)),
            SizedBox(height: 8),
            Text('Tap the camera button to scan one'),
          ],
        ),
      );
    }

    return RefreshIndicator(
      onRefresh: () => provider.loadReceipts(),
      child: ListView.builder(
        itemCount: provider.receipts.length,
        itemBuilder: (context, index) {
          final receipt = provider.receipts[index];
          return _ReceiptCard(receipt: receipt);
        },
      ),
    );
  }
}

class _ReceiptCard extends StatelessWidget {
  final Receipt receipt;
  const _ReceiptCard({required this.receipt});

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      child: ListTile(
        leading: Icon(
          receipt.ocrStatus == 'done' ? Icons.receipt : Icons.hourglass_top,
          color: receipt.ocrStatus == 'done' ? Colors.green : Colors.orange,
        ),
        title: Text(receipt.storeName ?? 'Unknown store'),
        subtitle: Text(receipt.purchaseDate ?? receipt.createdAt.substring(0, 10)),
        trailing: receipt.total != null
            ? Text(
                '${receipt.total!.toStringAsFixed(2)} ${receipt.currency ?? "NOK"}',
                style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 16),
              )
            : const Text('--'),
        onTap: () {
          Navigator.of(context).push(
            MaterialPageRoute(
              builder: (_) => ReceiptDetailScreen(receiptId: receipt.id),
            ),
          );
        },
      ),
    );
  }
}

class _ItemsTab extends StatelessWidget {
  const _ItemsTab();

  @override
  Widget build(BuildContext context) {
    return const ItemsScreen();
  }
}

class _AnalyticsTab extends StatelessWidget {
  const _AnalyticsTab();

  @override
  Widget build(BuildContext context) {
    return const Center(child: Text('Analytics - coming soon'));
  }
}
