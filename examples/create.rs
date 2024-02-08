use std::fs::{self, File};
use std::io::Result;
use std::path::Path;

fn main() -> std::io::Result<()> {
    // 定义要创建的目录列表
    let dirs = vec![
        "test/1-C",
        "test/2CH",
        "test/3-UC",
        "test/AVAV66.XYZ@BAB084/論壇文宣",
        "test/AVAV66.XYZ@NSFS158",
        "test/TEST1/trailers",
        "test/259LUXU-1699/UNCENSORED 123",
        "test/259LUXU-1699/trailers",
        "test/259LUXU-1277/trailers",
        "test/BMW-282/trailers",
    ];

    // 文件列表也加上 "test/" 前缀
    let files = [
        "test/2CH/1.nfo",
        "test/2CH/asd-C.mp4",
        "test/AVAV66.XYZ@BAB084/論壇文宣/灣搭拉咩拉@無限討論區 FastZone.ORG.txt",
        "test/TEST1/test1.mp4",
        "test/259LUXU-1699/259LUXU-1699.nfo",
        "test/259LUXU-1699/trailers/LUXU-1699-Trailer.strm",
        "test/259LUXU-1699/folder.jpg",
        "test/259LUXU-1699/backdrop.jpg",
        "test/259LUXU-1699/landscape.jpg",
        "test/259LUXU-1699/259LUXU-1699.mp4",
        "test/259LUXU-1277/11.mp4",
        "test/259LUXU-1277/111.mp4",
        "test/259LUXU-1277/UNCENSORED 123",
        "test/259LUXU-1277/259LUXU-1277.nfo",
        "test/259LUXU-1277/trailers/LUXU-1277-Trailer.strm",
        "test/259LUXU-1277/folder.jpg",
        "test/259LUXU-1277/backdrop.jpg",
        "test/259LUXU-1277/landscape.jpg",
        "test/259LUXU-1277/259LUXU-1277.mp4",
        "test/BMW-282/BMW-282-Trailer.strm",
        "test/BMW-282/bmw-282.nfo",
        "test/BMW-282/bmw-282-poster.jpg",
        "test/BMW-282/bmw-282-landscape.jpg",
        "test/BMW-282/bmw-282-backdrop.jpg",
        "test/BMW-282/bmw-282.mp4",
    ];

    // 首先确保 "test" 目录存在
    fs::create_dir_all("test")?;

    // 然后遍历目录列表并创建每个目录
    for dir in dirs {
        fs::create_dir_all(dir)?;
    }

    // 遍历文件列表并创建每个文件
    for file in files.iter() {
        File::create(Path::new(file))?;
    }

    println!("目录结构和文件已创建。");
    Ok(())
}
