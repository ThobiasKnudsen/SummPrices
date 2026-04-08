import '../models/receipt.dart';
import 'api_client.dart';

class ItemListResponse {
  final List<ItemWithContext> items;
  final int totalCount;

  ItemListResponse({required this.items, required this.totalCount});

  factory ItemListResponse.fromJson(Map<String, dynamic> json) {
    return ItemListResponse(
      items: (json['items'] as List).map((i) => ItemWithContext.fromJson(i)).toList(),
      totalCount: json['total_count'],
    );
  }
}

class ItemWithContext {
  final String id;
  final String receiptId;
  final String description;
  final double? quantity;
  final double? unitPrice;
  final double? lineTotal;
  final String? productCode;
  final String? storeName;
  final String? purchaseDate;

  ItemWithContext({
    required this.id,
    required this.receiptId,
    required this.description,
    this.quantity,
    this.unitPrice,
    this.lineTotal,
    this.productCode,
    this.storeName,
    this.purchaseDate,
  });

  factory ItemWithContext.fromJson(Map<String, dynamic> json) {
    return ItemWithContext(
      id: json['id'],
      receiptId: json['receipt_id'],
      description: json['description'],
      quantity: json['quantity']?.toDouble(),
      unitPrice: json['unit_price'] != null ? double.tryParse(json['unit_price'].toString()) : null,
      lineTotal: json['line_total'] != null ? double.tryParse(json['line_total'].toString()) : null,
      productCode: json['product_code'],
      storeName: json['store_name'],
      purchaseDate: json['purchase_date'],
    );
  }
}

class ItemService {
  final ApiClient _api;

  ItemService(this._api);

  Future<ItemListResponse> list({
    int page = 1,
    int perPage = 50,
    String? q,
    String? store,
    String? from,
    String? to,
  }) async {
    final response = await _api.dio.get('/api/items', queryParameters: {
      'page': page,
      'per_page': perPage,
      if (q != null && q.isNotEmpty) 'q': q,
      if (store != null) 'store': store,
      if (from != null) 'from': from,
      if (to != null) 'to': to,
    });
    return ItemListResponse.fromJson(response.data);
  }

  Future<Item> update(String id, Map<String, dynamic> data) async {
    final response = await _api.dio.put('/api/items/$id', data: data);
    return Item.fromJson(response.data);
  }

  Future<void> delete(String id) async {
    await _api.dio.delete('/api/items/$id');
  }
}
