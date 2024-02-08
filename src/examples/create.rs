use std::fs::{self, File};
use std::io::Result;
use std::path::Path;

fn main() -> std::io::Result<()> {
    // 定义要创建的目录列表
    let dirs = vec![
        "1-C",
        "2CH",
        "3-UC",
        "AVAV66.XYZ@BAB084/論壇文宣",
        "AVAV66.XYZ@NSFS158",
        "TEST1/trailers",
        "259LUXU-1699/UNCENSORED 123",
        "259LUXU-1699/trailers",
        "259LUXU-1277/trailers",
        "BMW-282/trailers",
    ];

    let files = [
        "2CH/1.nfo",
        "2CH/asd-C.mp4",
        "AVAV66.XYZ@BAB084/論壇文宣/灣搭拉咩拉@無限討論區 FastZone.ORG.txt",
        "TEST1/test1.mp4",
        "259LUXU-1699/259LUXU-1699.nfo",
        "259LUXU-1699/trailers/LUXU-1699-Trailer.strm",
        "259LUXU-1699/folder.jpg",
        "259LUXU-1699/backdrop.jpg",
        "259LUXU-1699/landscape.jpg",
        "259LUXU-1699/259LUXU-1699.mp4",
        "259LUXU-1277/11.mp4",
        "259LUXU-1277/111.mp4",
        "259LUXU-1277/UNCENSORED 123",
        "259LUXU-1277/259LUXU-1277.nfo",
        "259LUXU-1277/trailers/LUXU-1277-Trailer.strm",
        "259LUXU-1277/folder.jpg",
        "259LUXU-1277/backdrop.jpg",
        "259LUXU-1277/landscape.jpg",
        "259LUXU-1277/259LUXU-1277.mp4",
        "BMW-282/BMW-282-Trailer.strm",
        "BMW-282/bmw-282.nfo",
        "BMW-282/bmw-282-poster.jpg",
        "BMW-282/bmw-282-landscape.jpg",
        "BMW-282/bmw-282-backdrop.jpg",
        "BMW-282/bmw-282.mp4",
    ];

    // 遍历目录列表并创建每个目录
    for dir in dirs {
        fs::create_dir_all(dir)?;
    }

    for file in files.iter() {
        File::create(Path::new(file))?;
    }

    println!("目录结构和文件已创建。");
    Ok(())
}
