pub async fn get_mempool_info(
    node: web::Data<Arc<Node>>,
) -> Result<HttpResponse, ApiError> {
    let info = node.mempool().get_info();
    Ok(HttpResponse::Ok().json(info))
}