<?php
function search($con) {
    $id = $_GET['id'];
    $result = mysqli_query($con, "SELECT * FROM users WHERE id = '$id'");
    echo $result;
}
?>
