<?php
function search($con) {
    $id = $_GET['id'];
    $stmt = $con->prepare("SELECT * FROM users WHERE id = ?");
    $stmt->bind_param("s", $id);
    $stmt->execute();
    $result = $stmt->get_result();
    echo $result;
}
?>
